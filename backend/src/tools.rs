//! The agent's tools: specs the model sees, argument checks and dispatch.
//! Only names in `TOOL_NAMES` run. Tools return data, never sentences.

use std::time::Instant;

use genai::chat::Tool;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;
use time::Date;
use time::macros::format_description;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::catalog::{self, SERVICE_CATEGORIES, ServiceFilter, plan};
use crate::handoff;
use crate::locale::Locale;

pub const TOOL_NAMES: [&str; 5] =
    ["get_academy_info", "list_academy_events", "search_training_services", "calculate_training_plan", "transfer_to_director"];

/// Who is asking and where; filled in by the server, never by the model.
pub struct ToolContext<'a> {
    pub db: &'a PgPool,
    pub user: &'a CurrentUser,
    pub conversation_id: Uuid,
    pub run_id: Uuid,
    pub locale: Locale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolStatus {
    Ok,
    Rejected,
    Error,
}

impl ToolStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Rejected => "rejected",
            Self::Error => "error",
        }
    }
}

pub struct ToolOutcome {
    pub status: ToolStatus,
    pub result: Value,
    pub duration_ms: i32,
    /// Set when this call created or found a Director's Office case.
    pub ticket: Option<handoff::TicketSummary>,
}

pub fn specs(topics: &[String]) -> Vec<Tool> {
    vec![
        Tool::new("get_academy_info")
            .with_description(
                "Get one academy rule or procedure for trainers: service desk hours (受理時間), facility rules, booking and cancelling, \
                 clashes with classes and exams, race entry / off-campus paperwork (出賽申請), documents and certificates, trainer workshops (研習), \
                 Director's Office cases, dorm curfew (門禁), student support contacts (保健室).",
            )
            .with_schema(json!({
                "type": "object",
                "properties": { "topic": { "type": "string", "enum": topics } },
                "required": ["topic"]
            })),
        Tool::new("list_academy_events")
            .with_description(
                "List academy calendar entries (classes, exams, events, facility maintenance, workshops) and the G1/G2 race schedule \
                 between two dates. A race (kind 'race') takes a half month and has grade, racecourse, surface, distance and the \
                 career years that may enter (junior/classic/senior). Use for 'what is on', 'is there an exam', 'which G1 races are \
                 next month', or before planning a date. Dates are YYYY-MM-DD in academy (Japan) time.",
            )
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "date_from": { "type": "string", "description": "YYYY-MM-DD" },
                    "date_to": { "type": "string", "description": "YYYY-MM-DD, at most 62 days after date_from; default date_from" }
                },
                "required": ["date_from"]
            })),
        Tool::new("search_training_services")
            .with_description(
                "Find training facilities and services (tracks, pool, gym, rooms, timing equipment, video review, workshops, document service) \
                 with their slot length, capacity, demo fee and whether they are available. Use the trainer's own words; no exact name needed.",
            )
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Name or part of a name, in any language, e.g. '草地', '坂路', 'pool'" },
                    "category": { "type": "string", "enum": SERVICE_CATEGORIES },
                    "only_available": { "type": "boolean" }
                }
            })),
        Tool::new("calculate_training_plan")
            .with_description(
                "Calculate a training plan: total minutes and demo fees for services × sessions, and, with a date and start time, \
                 the schedule and clashes with the academy calendar, closed facilities and the 20:30 last use. \
                 Always use this for any total or time calculation; never add numbers yourself.",
            )
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "items": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "service": { "type": "string", "description": "The service in the trainer's own words, not translated" },
                                "sessions": { "type": "integer", "minimum": 1, "maximum": 50 },
                                "minutes_per_session": { "type": "integer", "minimum": 5, "maximum": 480, "description": "Only if the trainer gave a length" }
                            },
                            "required": ["service", "sessions"]
                        }
                    },
                    "date": { "type": "string", "description": "YYYY-MM-DD" },
                    "start_time": { "type": "string", "description": "HH:MM, 24-hour" }
                },
                "required": ["items"]
            })),
        Tool::new("transfer_to_director")
            .with_description(
                "Send the matter to the Director's Office, where a real person answers as Director Akikawa. Use when the trainer asks for \
                 the director or a person, makes a complaint, asks for an exception, or asks about the academy something no other tool can answer. \
                 Call it at most once per answer.",
            )
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "reason": { "type": "string", "enum": handoff::REASONS },
                    "summary": { "type": "string", "description": "What the trainer needs, for the Director's Office, in the trainer's language (max 500 characters)" }
                },
                "required": ["reason", "summary"]
            })),
    ]
}

fn invalid(detail: impl Into<String>) -> (ToolStatus, Value) {
    (ToolStatus::Rejected, json!({ "error": "invalid_arguments", "detail": detail.into() }))
}

fn parse_date(text: &str) -> Option<Date> {
    Date::parse(text.trim(), format_description!("[year]-[month]-[day]")).ok()
}

#[derive(Deserialize)]
struct InfoArgs {
    topic: String,
}

#[derive(Deserialize)]
struct EventArgs {
    date_from: String,
    date_to: Option<String>,
}

#[derive(Deserialize)]
struct TransferArgs {
    reason: String,
    summary: String,
}

/// Run one tool call. Unknown names and bad arguments are refused, not guessed.
pub async fn dispatch(ctx: &ToolContext<'_>, name: &str, arguments: &Value) -> ToolOutcome {
    let started = Instant::now();
    let mut ticket = None;
    let (status, result) = match name {
        "get_academy_info" => match serde_json::from_value::<InfoArgs>(arguments.clone()) {
            Err(e) => invalid(e.to_string()),
            Ok(a) => match catalog::get_info(ctx.db, &a.topic).await {
                Ok(Some(row)) => (ToolStatus::Ok, json!({ "found": true, "info": row.view(ctx.locale) })),
                Ok(None) => match catalog::info_topics(ctx.db).await {
                    Ok(topics) => (ToolStatus::Ok, json!({ "found": false, "note": format!("No academy information about '{}'.", a.topic), "topics_available": topics })),
                    Err(e) => db_error(e),
                },
                Err(e) => db_error(e),
            },
        },
        "list_academy_events" => match serde_json::from_value::<EventArgs>(arguments.clone()) {
            Err(e) => invalid(e.to_string()),
            Ok(a) => {
                let from = parse_date(&a.date_from);
                let to = a.date_to.as_deref().filter(|t| !t.is_empty()).map(parse_date).unwrap_or(from);
                match (from, to) {
                    (Some(from), Some(to)) if to >= from && (to - from).whole_days() <= 62 => match catalog::list_events(ctx.db, from, to).await {
                        Ok(rows) => {
                            let events: Vec<_> = rows.iter().map(|r| r.view(ctx.locale)).collect();
                            let note = if events.is_empty() { "No calendar entries in this range." } else { "" };
                            (ToolStatus::Ok, json!({ "found": !events.is_empty(), "count": events.len(), "date_from": from.to_string(), "date_to": to.to_string(), "events": events, "note": note }))
                        }
                        Err(e) => db_error(e),
                    },
                    _ => invalid("dates must be YYYY-MM-DD, date_to not before date_from, at most 62 days"),
                }
            }
        },
        "search_training_services" => match serde_json::from_value::<ServiceFilter>(arguments.clone()) {
            Err(e) => invalid(e.to_string()),
            Ok(filter) if filter.category.as_deref().is_some_and(|c| !c.is_empty() && !SERVICE_CATEGORIES.contains(&c)) => {
                invalid(format!("category must be one of {SERVICE_CATEGORIES:?}"))
            }
            Ok(filter) => match catalog::search_services(ctx.db, &filter).await {
                Ok(rows) => {
                    let items: Vec<_> = rows.iter().take(10).map(|r| r.view(ctx.locale)).collect();
                    if items.is_empty() {
                        (ToolStatus::Ok, json!({ "found": false, "items": [], "note": "No service matches. Do not guess; offer the list or the Director's Office." }))
                    } else {
                        (ToolStatus::Ok, json!({ "found": true, "count": items.len(), "items": items }))
                    }
                }
                Err(e) => db_error(e),
            },
        },
        "calculate_training_plan" => match serde_json::from_value::<plan::PlanInput>(arguments.clone()) {
            Err(e) => invalid(e.to_string()),
            Ok(input) => match plan::calculate_from_db(ctx.db, &input, ctx.locale).await {
                Ok(r) => (ToolStatus::Ok, serde_json::to_value(r).unwrap_or_default()),
                Err(plan::PlanError::Invalid(detail)) => invalid(detail),
                Err(plan::PlanError::Database(e)) => db_error(e),
            },
        },
        "transfer_to_director" => match serde_json::from_value::<TransferArgs>(arguments.clone()) {
            Err(e) => invalid(e.to_string()),
            Ok(a) if !handoff::REASONS.contains(&a.reason.as_str()) => invalid(format!("reason must be one of {:?}", handoff::REASONS)),
            Ok(a) if a.summary.trim().is_empty() => invalid("summary is empty"),
            Ok(a) => {
                let summary: String = a.summary.trim().chars().take(500).collect();
                let request = handoff::NewTicket {
                    requester: ctx.user,
                    conversation_id: Some(ctx.conversation_id),
                    reason: &a.reason,
                    summary: &summary,
                    origin: "agent",
                    // One case per answer, even if the model calls the tool twice.
                    idempotency_key: format!("run:{}", ctx.run_id),
                };
                match handoff::create_ticket(ctx.db, &request).await {
                    Ok((t, created)) => {
                        let result = json!({
                            "created": created,
                            "ticket_no": t.ticket_no,
                            "status": t.status,
                            "note": "The case is waiting for a real person at the Director's Office. Tell the trainer the case number and that they will be \
                                     answered here. Do not promise an outcome, and do not call this tool again.",
                        });
                        ticket = Some(t);
                        (ToolStatus::Ok, result)
                    }
                    Err(e) => db_error(e),
                }
            }
        },
        _ => (ToolStatus::Rejected, json!({ "error": "unknown_tool", "detail": format!("'{name}' is not a tool of this system"), "tools": TOOL_NAMES })),
    };
    ToolOutcome { status, result, duration_ms: started.elapsed().as_millis().min(i32::MAX as u128) as i32, ticket }
}

fn db_error(e: sqlx::Error) -> (ToolStatus, Value) {
    tracing::error!(error = %e, "tool database error");
    (ToolStatus::Error, json!({ "error": "internal", "detail": "The academy database did not answer. Apologise and offer the Director's Office." }))
}
