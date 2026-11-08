//! MCP server for outside agents (docs/engineering/upgrade-plan-1.1.md §6). A thin adapter: each MCP
//! tool calls an existing service function, over the public views of migration 0007, so outside
//! agents only ever see `visibility = public` rows. Which tools exist comes from the skills registry.
//!
//! - stdio: `tracen-api mcp-stdio` for local clients (Claude Desktop, Claude Code, self-hosted agents).
//! - Streamable HTTP: `/mcp` on the API, off until an admin switches it on, and only with the
//!   read-only `tracen_mcp` database role (MCP_DATABASE_URL).

pub mod admin;
pub mod eval;
pub mod export;
pub mod limits;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use axum::http::request::Parts;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
    ToolAnnotations,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use time::Date;
use time::macros::format_description;
use uuid::Uuid;

use crate::catalog::{self, PUBLIC_VIEWS, SERVICE_CATEGORIES, ServiceFilter, plan};
use crate::config::Config;
use crate::feedback::{self, NewFeedback, Refusal};
use crate::handbook::{self, Audience, SearchRequest, Source};
use crate::llm::Ai;
use crate::locale::Locale;
use crate::settings::{self, AiSettings, BudgetPolicy, UsageRecord};
use crate::skills;

use self::limits::{DAY, Limits, MINUTE};

pub const INSTRUCTIONS: &str = "\
Tracen Academy public data from \"AI Hayakawa Tazuna\", an unofficial doujin (fan) project. Everything here is a demo setting \
of this system (source_type project or reference) or community game data (source_type community, see source_ref); it is never \
official Cygames material and never the rules of a real school.
- Rules and procedures: search_public_rules, and cite the section number (for example §3.2).
- Dates, exams, events and the G1/G2 race schedule: list_public_events.
- Facilities and services: search_public_services.
- Any total of minutes or fees, an end time or a clash check: calculate_training_plan. Never add numbers yourself.
- If a tool says found:false, say it was not found. Do not guess, and do not speak for the academy or the Director's Office.
- submit_public_feedback is only for non-personal reports about this public data. Never include names, contact details, \
student numbers or account details.
This server cannot read conversations, cases, accounts or memories, and cannot contact a person.";

// ---------------------------------------------------------------------------
// Settings (app_settings key `mcp`; read through the mcp_settings view)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct McpSettings {
    /// Remote HTTP access; stdio is a local process and does not depend on it.
    pub enabled: bool,
    /// Month-to-date cap for MCP query embeddings; above it search falls back to keywords.
    pub monthly_budget_usd: Option<f64>,
    pub read_per_minute: u32,
    pub key_read_per_minute: u32,
    pub feedback_per_ip_day: u32,
    pub key_feedback_per_day: u32,
    pub feedback_global_day: u32,
}

impl Default for McpSettings {
    fn default() -> Self {
        Self { enabled: false, monthly_budget_usd: Some(1.0), read_per_minute: 30, key_read_per_minute: 120, feedback_per_ip_day: 5, key_feedback_per_day: 20, feedback_global_day: 100 }
    }
}

impl McpSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.monthly_budget_usd.is_some_and(|b| !(0.0..=10_000.0).contains(&b)) {
            return Err("monthly_budget_usd must be 0 to 10000".into());
        }
        let limits = [self.read_per_minute, self.key_read_per_minute, self.feedback_per_ip_day, self.key_feedback_per_day, self.feedback_global_day];
        if limits.iter().any(|&n| n > 100_000) {
            return Err("limits must be 0 to 100000".into());
        }
        Ok(())
    }
}

pub const SETTINGS_KEY: &str = "mcp";

pub async fn load_settings(db: &PgPool) -> sqlx::Result<McpSettings> {
    let stored: Option<Value> = sqlx::query_scalar("SELECT value FROM mcp_settings WHERE key = $1").bind(SETTINGS_KEY).fetch_optional(db).await?;
    Ok(stored.and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default())
}

// ---------------------------------------------------------------------------
// The handler
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    Stdio,
    Http,
}

impl Transport {
    fn as_str(self) -> &'static str {
        match self {
            Self::Stdio => "stdio",
            Self::Http => "http",
        }
    }
}

/// Who is calling, as far as the server can tell.
#[derive(Debug, Clone)]
pub struct Caller {
    pub client_id: Option<Uuid>,
    pub scopes: Vec<String>,
    /// Rate-limit key: `client:<id>`, `ip:<address>`, or `stdio`. Kept in memory only.
    pub limit_key: String,
}

impl Caller {
    pub fn local() -> Self {
        Self { client_id: None, scopes: vec!["public:read".into(), "feedback:write".into()], limit_key: "stdio".into() }
    }
}

struct Inner {
    /// The read-only role's pool when MCP_DATABASE_URL is set; otherwise the main pool (stdio only).
    db: PgPool,
    ai: Arc<Ai>,
    config: Arc<Config>,
    transport: Transport,
    limits: Limits,
    /// Header that carries the client address behind a reverse proxy (MCP_CLIENT_IP_HEADER).
    ip_header: Option<String>,
}

#[derive(Clone)]
pub struct TracenMcp {
    inner: Arc<Inner>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CallStatus {
    Ok,
    Rejected,
    Error,
    RateLimited,
    Disabled,
}

impl CallStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Rejected => "rejected",
            Self::Error => "error",
            Self::RateLimited => "rate_limited",
            Self::Disabled => "disabled",
        }
    }
}

fn object(v: Value) -> Arc<Map<String, Value>> {
    Arc::new(v.as_object().cloned().unwrap_or_default())
}

fn locale_schema() -> Value {
    json!({ "type": "string", "enum": ["zh-TW", "ja", "en-GB"], "description": "Language of the returned text; default en-GB" })
}

/// Every MCP tool this server knows; `list_tools` offers those the registry enables.
fn all_tools() -> Vec<Tool> {
    let read = ToolAnnotations::new().read_only(true).destructive(false);
    vec![
        Tool::new(
            "search_public_rules",
            "Search the public sections of the Tracen Academy Trainer Handbook and academy information by meaning. Returns whole sections \
             with their number (e.g. §3.2), source_type and source_ref; found:false means nothing public covers the question.",
            object(json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "The question in the user's own words (any language)", "maxLength": 300 },
                    "top_k": { "type": "integer", "minimum": 1, "maximum": handbook::MAX_TOP_K },
                    "locale": locale_schema()
                },
                "required": ["query"]
            })),
        )
        .annotate(read.clone()),
        Tool::new(
            "list_public_events",
            "List public Tracen Academy calendar entries (classes, exams, events, maintenance, workshops) and the G1/G2 race schedule \
             between two dates (YYYY-MM-DD, academy time, at most 62 days).",
            object(json!({
                "type": "object",
                "properties": {
                    "date_from": { "type": "string", "description": "YYYY-MM-DD" },
                    "date_to": { "type": "string", "description": "YYYY-MM-DD; default date_from" },
                    "locale": locale_schema()
                },
                "required": ["date_from"]
            })),
        )
        .annotate(read.clone()),
        Tool::new(
            "search_public_services",
            "Find public Tracen Academy training facilities and services: name, category, slot length, capacity, demo fee, availability and location.",
            object(json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Facility in the user's words, any language, e.g. 'pool', '草地'" },
                    "category": { "type": "string", "enum": SERVICE_CATEGORIES },
                    "locale": locale_schema()
                }
            })),
        )
        .annotate(read.clone()),
        Tool::new(
            "calculate_training_plan",
            "Calculate total minutes and demo fees for public services × sessions and, with a date and start time, the schedule and clashes \
             with the public calendar, closed facilities and the 21:30 last use. Use it for any total; never add numbers yourself.",
            object(json!({
                "type": "object",
                "properties": {
                    "items": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "service": { "type": "string" },
                                "sessions": { "type": "integer", "minimum": 1, "maximum": 50 },
                                "minutes_per_session": { "type": "integer", "minimum": 5, "maximum": 480 }
                            },
                            "required": ["service", "sessions"]
                        }
                    },
                    "date": { "type": "string", "description": "YYYY-MM-DD" },
                    "start_time": { "type": "string", "description": "HH:MM, 24-hour" },
                    "locale": locale_schema()
                },
                "required": ["items"]
            })),
        )
        .annotate(read),
        Tool::new(
            "submit_public_feedback",
            "Send non-personal feedback about this public data (an error, missing information or a suggestion) to a human review queue. \
             Never include names, contact details, student numbers or account details: such feedback is refused. Returns a receipt number; \
             it does not change any data by itself.",
            object(json!({
                "type": "object",
                "properties": {
                    "category": { "type": "string", "enum": feedback::CATEGORIES },
                    "text": { "type": "string", "minLength": 10, "maxLength": 1000 },
                    "language": { "type": "string", "enum": feedback::LANGUAGES },
                    "related_ref": { "type": "string", "description": "Optional: a handbook section such as §3.2, or an id such as ev_midterm" },
                    "idempotency_key": { "type": "string", "description": "A new random value per feedback; resending with the same key does not create a second one" }
                },
                "required": ["category", "text", "language", "idempotency_key"]
            })),
        )
        .annotate(ToolAnnotations::new().read_only(false).destructive(false)),
    ]
}

fn tool_name(t: &Tool) -> &str {
    t.name.as_ref()
}

fn locale_arg(args: &Value) -> Locale {
    args.get("locale").and_then(Value::as_str).and_then(Locale::parse).unwrap_or(Locale::EnGb)
}

fn parse_date(text: &str) -> Option<Date> {
    Date::parse(text.trim(), format_description!("[year]-[month]-[day]")).ok()
}

fn refused(error: &str, detail: impl Into<String>) -> (CallStatus, Value) {
    (CallStatus::Rejected, json!({ "error": error, "detail": detail.into() }))
}

fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

impl TracenMcp {
    pub fn new(db: PgPool, ai: Arc<Ai>, config: Arc<Config>, transport: Transport) -> Self {
        let ip_header = std::env::var("MCP_CLIENT_IP_HEADER").ok().map(|h| h.trim().to_lowercase()).filter(|h| !h.is_empty());
        Self { inner: Arc::new(Inner { db, ai, config, transport, limits: Limits::default(), ip_header }) }
    }

    /// The tools the registry offers right now.
    pub fn tools(&self) -> Vec<Tool> {
        let offered = skills::registry().mcp_tools();
        all_tools().into_iter().filter(|t| offered.contains(&tool_name(t))).collect()
    }

    /// Identify an HTTP caller from the request: a client key if one is sent, else its address.
    async fn caller(&self, ctx: &RequestContext<RoleServer>) -> Result<Caller, (CallStatus, Value)> {
        if self.inner.transport == Transport::Stdio {
            return Ok(Caller::local());
        }
        let Some(parts) = ctx.extensions.get::<Parts>() else { return Err(refused("no_request", "the HTTP request was not available")) };
        let header = |name: &str| parts.headers.get(name).and_then(|v| v.to_str().ok()).map(str::trim).map(str::to_string);
        if let Some(auth) = header("authorization") {
            let Some(key) = auth.strip_prefix("Bearer ").map(str::trim).filter(|k| !k.is_empty()) else {
                return Err(refused("invalid_client_key", "use 'Authorization: Bearer <key>', or no header at all"));
            };
            #[derive(sqlx::FromRow)]
            struct Key {
                id: Uuid,
                scopes: Vec<String>,
                disabled_at: Option<time::OffsetDateTime>,
            }
            let found = sqlx::query_as::<_, Key>("SELECT id, scopes, disabled_at FROM mcp_client_keys WHERE key_hash = $1")
                .bind(sha256_hex(key))
                .fetch_optional(&self.inner.db)
                .await
                .map_err(|_| (CallStatus::Error, json!({ "error": "internal" })))?;
            return match found {
                Some(k) if k.disabled_at.is_none() => {
                    let _ = sqlx::query("UPDATE mcp_clients SET last_used_at = now() WHERE id = $1").bind(k.id).execute(&self.inner.db).await;
                    Ok(Caller { client_id: Some(k.id), scopes: k.scopes, limit_key: format!("client:{}", k.id) })
                }
                _ => Err(refused("invalid_client_key", "this client key is unknown or disabled")),
            };
        }
        let forwarded = self.inner.ip_header.as_deref().and_then(header).and_then(|v| v.split(',').next().map(|s| s.trim().to_string())).filter(|s| !s.is_empty());
        let direct = parts.extensions.get::<axum::extract::ConnectInfo<SocketAddr>>().map(|c| c.0.ip().to_string());
        let ip = forwarded.or(direct).unwrap_or_else(|| "unknown".into());
        Ok(Caller { client_id: None, scopes: vec!["public:read".into(), "feedback:write".into()], limit_key: format!("ip:{ip}") })
    }

    /// Run one tool for `caller`. Shared by MCP calls and `eval-mcp`.
    pub async fn run_tool(&self, name: &str, args: &Value, caller: &Caller) -> (CallStatus, Value) {
        let offered = skills::registry().mcp_tools();
        if !offered.contains(&name) {
            return refused("unknown_tool", format!("'{name}' is not a tool of this server"));
        }
        let mcp = load_settings(&self.inner.db).await.unwrap_or_default();
        if self.inner.transport == Transport::Http && !mcp.enabled {
            return (CallStatus::Disabled, json!({ "error": "disabled", "detail": "The academy has switched remote MCP access off." }));
        }
        let writes = skills::MCP_WRITE_TOOLS.contains(&name);
        if self.inner.transport == Transport::Http {
            let keyed = caller.client_id.is_some();
            let ok = if writes {
                let per_caller = if keyed { mcp.key_feedback_per_day } else { mcp.feedback_per_ip_day };
                self.inner.limits.allow(&format!("fb:{}", caller.limit_key), DAY, per_caller) && self.inner.limits.allow("fb:all", DAY, mcp.feedback_global_day)
            } else {
                self.inner.limits.allow(&format!("read:{}", caller.limit_key), MINUTE, if keyed { mcp.key_read_per_minute } else { mcp.read_per_minute })
            };
            if !ok {
                return (CallStatus::RateLimited, json!({ "error": "rate_limited", "detail": "Too many requests; try again later." }));
            }
        }
        let scope = if writes { "feedback:write" } else { "public:read" };
        if !caller.scopes.iter().any(|s| s == scope) {
            return refused("forbidden", format!("this client key has no '{scope}' scope"));
        }
        match name {
            "search_public_rules" => self.search_rules(args, &mcp).await,
            "list_public_events" => self.list_events(args).await,
            "search_public_services" => self.search_services(args).await,
            "calculate_training_plan" => self.calculate(args).await,
            "submit_public_feedback" => self.submit_feedback(args, caller).await,
            _ => refused("unknown_tool", format!("'{name}' is not a tool of this server")),
        }
    }

    async fn ai_settings(&self) -> Result<AiSettings, (CallStatus, Value)> {
        settings::load_public(&self.inner.db, &self.inner.config).await.map_err(|e| {
            tracing::error!(error = %e, "mcp: settings");
            (CallStatus::Error, json!({ "error": "internal" }))
        })
    }

    /// Embeddings are allowed while both the MCP cap and the overall blocking budget have room.
    async fn embedding_allowed(&self, ai: &AiSettings, mcp: &McpSettings) -> bool {
        let db = &self.inner.db;
        if let Some(cap) = mcp.monthly_budget_usd
            && settings::spent_this_month_public(db, Some("mcp_embed")).await.unwrap_or(f64::MAX) >= cap
        {
            return false;
        }
        if let Some(limit) = ai.monthly_budget_usd
            && ai.on_budget_exceeded == BudgetPolicy::Block
            && settings::spent_this_month_public(db, None).await.unwrap_or(f64::MAX) >= limit
        {
            return false;
        }
        true
    }

    async fn search_rules(&self, args: &Value, mcp: &McpSettings) -> (CallStatus, Value) {
        let Some(query) = args.get("query").and_then(Value::as_str).map(str::trim).filter(|q| !q.is_empty()) else {
            return refused("invalid_arguments", "query is required");
        };
        let query: String = query.chars().take(300).collect();
        let ai = match self.ai_settings().await {
            Ok(s) => s,
            Err(e) => return e,
        };
        let request = SearchRequest {
            query: &query,
            top_k: args.get("top_k").and_then(Value::as_u64).map_or(handbook::DEFAULT_TOP_K, |k| k as usize),
            locale: locale_arg(args),
            audience: Audience::Public,
            user_id: None,
            purpose: "mcp_embed",
            max_distance: None,
            source: Source::PublicView,
            allow_embed: self.embedding_allowed(&ai, mcp).await,
        };
        match handbook::search(&self.inner.db, &self.inner.ai, &ai, &request).await {
            Ok(o) => {
                if o.embed_tokens > 0 {
                    settings::record_usage(
                        &self.inner.db,
                        &ai,
                        UsageRecord { purpose: "mcp_embed", model: &ai.embed_model, prompt_tokens: o.embed_tokens, completion_tokens: 0, user_id: None, run_id: None },
                    )
                    .await;
                }
                let results: Vec<Value> = o
                    .results
                    .iter()
                    .map(|h| json!({ "section": h.section, "title": h.title, "chapter": h.chapter, "content": h.content, "lang": h.lang, "source_type": h.source_type, "source_ref": h.source_ref }))
                    .collect();
                let note = if o.found {
                    "Closest public sections; they may not answer the question. Cite the section number. Demo setting of an unofficial fan project."
                } else {
                    "Nothing public covers this. Say it was not found; do not guess."
                };
                (CallStatus::Ok, json!({ "found": o.found, "method": o.method, "results": results, "note": note }))
            }
            Err(e) => {
                tracing::error!(error = ?e, "mcp: search");
                (CallStatus::Error, json!({ "error": "internal", "detail": "The search did not answer." }))
            }
        }
    }

    async fn list_events(&self, args: &Value) -> (CallStatus, Value) {
        let locale = locale_arg(args);
        let from = args.get("date_from").and_then(Value::as_str).and_then(parse_date);
        let to = match args.get("date_to").and_then(Value::as_str).filter(|t| !t.is_empty()) {
            Some(t) => parse_date(t),
            None => from,
        };
        let (Some(from), Some(to)) = (from, to) else { return refused("invalid_arguments", "dates must be YYYY-MM-DD") };
        if to < from || (to - from).whole_days() > 62 {
            return refused("invalid_arguments", "date_to must not be before date_from, and the range is at most 62 days");
        }
        match catalog::list_events_in(&self.inner.db, from, to, PUBLIC_VIEWS).await {
            Ok(rows) => {
                let events: Vec<_> = rows.iter().map(|r| r.view(locale)).collect();
                (CallStatus::Ok, json!({ "found": !events.is_empty(), "count": events.len(), "date_from": from.to_string(), "date_to": to.to_string(), "events": events }))
            }
            Err(e) => {
                tracing::error!(error = %e, "mcp: events");
                (CallStatus::Error, json!({ "error": "internal" }))
            }
        }
    }

    async fn search_services(&self, args: &Value) -> (CallStatus, Value) {
        let locale = locale_arg(args);
        let category = args.get("category").and_then(Value::as_str).map(str::to_string).filter(|c| !c.is_empty());
        if category.as_deref().is_some_and(|c| !SERVICE_CATEGORIES.contains(&c)) {
            return refused("invalid_arguments", format!("category must be one of {SERVICE_CATEGORIES:?}"));
        }
        let filter = ServiceFilter { query: args.get("query").and_then(Value::as_str).map(str::to_string), category, only_available: None };
        match catalog::search_services_in(&self.inner.db, &filter, PUBLIC_VIEWS).await {
            Ok(rows) => {
                // Only the listed fields leave the server (§6.4).
                let items: Vec<Value> = rows
                    .iter()
                    .take(10)
                    .map(|r| {
                        json!({
                            "id": r.id, "name": r.localized_name(locale), "category": r.category, "unit_minutes": r.unit_minutes,
                            "capacity": r.capacity, "fee_jpy": r.fee_jpy, "available": r.available, "location": r.location, "source_type": r.source_type
                        })
                    })
                    .collect();
                (CallStatus::Ok, json!({ "found": !items.is_empty(), "count": items.len(), "items": items, "note": "Fees are demo values of this system." }))
            }
            Err(e) => {
                tracing::error!(error = %e, "mcp: services");
                (CallStatus::Error, json!({ "error": "internal" }))
            }
        }
    }

    async fn calculate(&self, args: &Value) -> (CallStatus, Value) {
        let mut input = args.clone();
        if let Some(o) = input.as_object_mut() {
            o.remove("locale");
        }
        let input: plan::PlanInput = match serde_json::from_value(input) {
            Ok(i) => i,
            Err(e) => return refused("invalid_arguments", e.to_string()),
        };
        match plan::calculate_in(&self.inner.db, &input, locale_arg(args), PUBLIC_VIEWS).await {
            Ok(r) => (CallStatus::Ok, serde_json::to_value(r).unwrap_or_default()),
            Err(plan::PlanError::Invalid(d)) => refused("invalid_arguments", d),
            Err(plan::PlanError::Database(e)) => {
                tracing::error!(error = %e, "mcp: plan");
                (CallStatus::Error, json!({ "error": "internal" }))
            }
        }
    }

    async fn submit_feedback(&self, args: &Value, caller: &Caller) -> (CallStatus, Value) {
        let f: NewFeedback = match serde_json::from_value(args.clone()) {
            Ok(f) => f,
            Err(e) => return refused("invalid_arguments", e.to_string()),
        };
        let (text, flags) = match feedback::check(&f) {
            Ok(t) => t,
            Err(Refusal::Invalid(d)) => return refused("invalid_arguments", d),
            Err(Refusal::PersonalData(what)) => {
                return refused("contains_personal_data", format!("The text looks like it contains a {what}. Remove personal data and send it again."));
            }
        };
        match feedback::submit(&self.inner.db, caller.client_id, &f, &text, &flags).await {
            Ok(r) => (CallStatus::Ok, serde_json::to_value(r).unwrap_or_default()),
            Err(e) => {
                tracing::error!(error = %e, "mcp: feedback");
                (CallStatus::Error, json!({ "error": "internal" }))
            }
        }
    }

    /// One row per call: client, tool, status, time and argument length; never the arguments.
    async fn log_call(&self, caller: Option<&Caller>, tool: &str, status: CallStatus, started: Instant, argument_chars: usize) {
        let r = sqlx::query("INSERT INTO mcp_calls (client_id, transport, tool, status, duration_ms, argument_chars) VALUES ($1, $2, $3, $4, $5, $6)")
            .bind(caller.and_then(|c| c.client_id))
            .bind(self.inner.transport.as_str())
            .bind(tool.chars().take(64).collect::<String>())
            .bind(status.as_str())
            .bind(started.elapsed().as_millis().min(i32::MAX as u128) as i32)
            .bind(argument_chars.min(i32::MAX as usize) as i32)
            .execute(&self.inner.db)
            .await;
        if let Err(e) = r {
            tracing::warn!(error = %e, "mcp: could not log the call");
        }
    }
}

impl ServerHandler for TracenMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("tracen-academy-public", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }

    async fn list_tools(&self, _request: Option<PaginatedRequestParams>, _context: RequestContext<RoleServer>) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(self.tools()))
    }

    async fn call_tool(&self, request: CallToolRequestParams, context: RequestContext<RoleServer>) -> Result<CallToolResponse, McpError> {
        let started = Instant::now();
        let name = request.name.to_string();
        let args = Value::Object(request.arguments.unwrap_or_default());
        let argument_chars = args.to_string().chars().count();
        let (caller, (status, value)) = match self.caller(&context).await {
            Ok(caller) => {
                let outcome = self.run_tool(&name, &args, &caller).await;
                (Some(caller), outcome)
            }
            Err(e) => (None, e),
        };
        self.log_call(caller.as_ref(), &name, status, started, argument_chars).await;
        let result = if status == CallStatus::Ok { CallToolResult::structured(value) } else { CallToolResult::structured_error(value) };
        Ok(result.into())
    }
}

// ---------------------------------------------------------------------------
// Transports
// ---------------------------------------------------------------------------

/// The pool MCP uses: the read-only role when MCP_DATABASE_URL is set.
pub async fn connect_readonly() -> anyhow::Result<Option<PgPool>> {
    let Ok(url) = std::env::var("MCP_DATABASE_URL") else { return Ok(None) };
    if url.trim().is_empty() {
        return Ok(None);
    }
    let pool = sqlx::postgres::PgPoolOptions::new().max_connections(5).connect(&url).await.map_err(|e| anyhow::anyhow!("connect MCP_DATABASE_URL: {e}"))?;
    Ok(Some(pool))
}

/// Serve MCP over stdin/stdout until the client disconnects. Logs go to stderr.
pub async fn serve_stdio(handler: TracenMcp) -> anyhow::Result<()> {
    use rmcp::ServiceExt;
    let running = handler.serve(rmcp::transport::stdio()).await.map_err(|e| anyhow::anyhow!("start MCP over stdio: {e}"))?;
    running.waiting().await.map_err(|e| anyhow::anyhow!("MCP over stdio: {e}"))?;
    Ok(())
}

/// The Streamable HTTP service for `/mcp`. Hosts default to loopback; a browser `Origin` is refused
/// unless MCP_ALLOWED_ORIGINS lists it, which blocks DNS-rebinding pages (§6.3).
pub fn http_service(handler: TracenMcp) -> rmcp::transport::streamable_http_server::StreamableHttpService<TracenMcp, rmcp::transport::streamable_http_server::session::local::LocalSessionManager> {
    use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
    use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
    let list = |key: &str| -> Vec<String> { std::env::var(key).unwrap_or_default().split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect() };
    let mut config = StreamableHttpServerConfig::default();
    let hosts = list("MCP_ALLOWED_HOSTS");
    if !hosts.is_empty() {
        config = config.with_allowed_hosts(hosts);
    }
    let origins = list("MCP_ALLOWED_ORIGINS");
    config = if origins.is_empty() { config.enforce_origin_validation() } else { config.with_allowed_origins(origins) };
    StreamableHttpService::new(move || Ok(handler.clone()), Arc::new(LocalSessionManager::default()), config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registry_mcp_tool_has_a_definition() {
        let defined: Vec<String> = all_tools().iter().map(|t| tool_name(t).to_string()).collect();
        for t in skills::registry().mcp_tools() {
            assert!(defined.iter().any(|d| d == t), "{t} has no MCP definition");
        }
        assert!(!defined.iter().any(|d| d == "transfer_to_director"));
    }

    #[test]
    fn only_feedback_writes() {
        for t in all_tools() {
            let read_only = t.annotations.as_ref().and_then(|a| a.read_only_hint).unwrap_or(false);
            assert_eq!(read_only, !skills::MCP_WRITE_TOOLS.contains(&tool_name(&t)), "{}", tool_name(&t));
        }
    }

    #[test]
    fn default_settings_keep_remote_access_off() {
        let s = McpSettings::default();
        assert!(!s.enabled);
        assert!(s.validate().is_ok());
        assert!(McpSettings { monthly_budget_usd: Some(-1.0), ..McpSettings::default() }.validate().is_err());
    }
}
