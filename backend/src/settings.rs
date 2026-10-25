//! AI settings and the monthly budget, adjustable on the management site (no redeploy).
//! Prices are entered by an admin; nothing here guesses a provider's price list.

use std::collections::BTreeMap;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::{Date, OffsetDateTime, Time};
use uuid::Uuid;

use crate::auth::{AdminUser, StaffUser};
use crate::catalog::{ACADEMY_OFFSET, academy_now};
use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

const KEY: &str = "ai";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetPolicy {
    /// Stop AI calls for the rest of the month; manual pages keep working.
    Block,
    /// Keep going and only show the overrun on the management site.
    Warn,
}

/// How long replies should be; the prompt turns it into an instruction, not a hard cut.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplyLength {
    Brief,
    /// As long as the question needs.
    #[default]
    Natural,
    Detailed,
}

/// How far Tazuna follows the trainer off academy matters before steering back.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SmallTalk {
    /// Decline in one sentence and offer academy help.
    Off,
    /// Chat for a few exchanges, then bring the talk back.
    #[default]
    Balanced,
    /// Chat as long as the trainer likes; mention work only at a natural opening.
    Relaxed,
}

/// The model's thinking depth. `Auto` sends nothing and leaves it to the provider.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reasoning {
    #[default]
    Auto,
    Low,
    Medium,
    High,
}

/// How the agent talks and thinks; adjustable on the management site.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ConversationSettings {
    pub reply_length: ReplyLength,
    pub small_talk: SmallTalk,
    pub reasoning: Reasoning,
    /// `None` = the provider's default. Google asks to keep Gemini 3 at its default of 1.0.
    pub temperature: Option<f64>,
    /// Upper limit per model call. On Gemini it includes thinking tokens, so keep it generous.
    pub max_output_tokens: u32,
    /// Model calls per run (tool rounds plus the final reply).
    pub max_steps: u32,
}

impl Default for ConversationSettings {
    fn default() -> Self {
        Self { reply_length: ReplyLength::default(), small_talk: SmallTalk::default(), reasoning: Reasoning::default(), temperature: None, max_output_tokens: 8192, max_steps: 6 }
    }
}

impl ConversationSettings {
    pub const MAX_STEPS: u32 = 12;

    fn validate(&self) -> Result<(), String> {
        if !(1..=Self::MAX_STEPS).contains(&self.max_steps) {
            return Err(format!("max_steps must be 1 to {}", Self::MAX_STEPS));
        }
        if !(256..=65_536).contains(&self.max_output_tokens) {
            return Err("max_output_tokens must be 256 to 65536".into());
        }
        if self.temperature.is_some_and(|t| !(0.0..=2.0).contains(&t)) {
            return Err("temperature must be 0 to 2".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Price {
    pub input_usd_per_mtok: f64,
    pub output_usd_per_mtok: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiSettings {
    /// Model for the agent loop; `mock` is the offline router.
    pub chat_model: String,
    /// Model for memory curation and conversation summaries.
    pub memory_model: String,
    /// Embedding model (768 dimensions are requested).
    pub embed_model: String,
    /// Model that turns recorded speech into text.
    pub transcribe_model: String,
    pub memory_enabled: bool,
    /// `None` = no limit.
    pub monthly_budget_usd: Option<f64>,
    pub on_budget_exceeded: BudgetPolicy,
    pub prices: BTreeMap<String, Price>,
    pub conversation: ConversationSettings,
}

impl AiSettings {
    /// Starting values before an admin changes anything; from .env.
    pub fn defaults(config: &Config) -> Self {
        let offline = config.model == "mock";
        // Offline means offline: with MODEL=mock every model is mock, whatever else .env says.
        let env = |key: &str, fallback: &str| {
            if offline {
                return "mock".to_string();
            }
            std::env::var(key).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| fallback.to_string())
        };
        Self {
            chat_model: config.model.clone(),
            memory_model: env("MEMORY_MODEL", &config.model),
            embed_model: env("EMBED_MODEL", if offline { "mock" } else { "gemini-embedding-2" }),
            transcribe_model: env("TRANSCRIBE_MODEL", if offline { "mock" } else { &config.model }),
            memory_enabled: true,
            monthly_budget_usd: None,
            on_budget_exceeded: BudgetPolicy::Block,
            prices: BTreeMap::new(),
            conversation: ConversationSettings { max_steps: (config.max_steps as u32).clamp(1, ConversationSettings::MAX_STEPS), ..ConversationSettings::default() },
        }
    }

    fn validate(&self) -> Result<(), String> {
        for (name, value) in [("chat_model", &self.chat_model), ("memory_model", &self.memory_model), ("embed_model", &self.embed_model), ("transcribe_model", &self.transcribe_model)] {
            let ok = !value.is_empty() && value.len() <= 100 && value.chars().all(|c| c.is_ascii_alphanumeric() || "-._:/".contains(c));
            if !ok {
                return Err(format!("{name} is not a valid model name"));
            }
        }
        if self.monthly_budget_usd.is_some_and(|b| !(0.0..=100_000.0).contains(&b)) {
            return Err("monthly_budget_usd must be 0 to 100000".into());
        }
        if self.prices.values().any(|p| !(0.0..=1000.0).contains(&p.input_usd_per_mtok) || !(0.0..=1000.0).contains(&p.output_usd_per_mtok)) {
            return Err("prices must be 0 to 1000 USD per million tokens".into());
        }
        self.conversation.validate()
    }
}

pub async fn load(db: &PgPool, config: &Config) -> sqlx::Result<AiSettings> {
    let stored: Option<Value> = sqlx::query_scalar("SELECT value FROM app_settings WHERE key = $1").bind(KEY).fetch_optional(db).await?;
    let defaults = AiSettings::defaults(config);
    Ok(match stored {
        // Fields missing from an older stored value fall back to the defaults.
        Some(v) => {
            let mut merged = serde_json::to_value(&defaults).unwrap_or_default();
            if let (Some(base), Some(over)) = (merged.as_object_mut(), v.as_object()) {
                for (k, val) in over {
                    base.insert(k.clone(), val.clone());
                }
            }
            serde_json::from_value(merged).unwrap_or(defaults)
        }
        None => defaults,
    })
}

fn month_start(now: OffsetDateTime) -> OffsetDateTime {
    let date = Date::from_calendar_date(now.year(), now.month(), 1).unwrap_or(now.date());
    date.with_time(Time::MIDNIGHT).assume_offset(ACADEMY_OFFSET)
}

pub async fn spent_this_month(db: &PgPool) -> sqlx::Result<f64> {
    sqlx::query_scalar("SELECT COALESCE(SUM(cost_usd), 0)::float8 FROM model_usage WHERE occurred_at >= $1")
        .bind(month_start(academy_now()))
        .fetch_one(db)
        .await
}

/// `Err` when the month's budget is used up and the policy is to block.
pub async fn check_budget(db: &PgPool, settings: &AiSettings) -> Result<(), BudgetExceeded> {
    let Some(limit) = settings.monthly_budget_usd else { return Ok(()) };
    let spent = spent_this_month(db).await.map_err(|_| BudgetExceeded)?;
    if spent >= limit && settings.on_budget_exceeded == BudgetPolicy::Block { Err(BudgetExceeded) } else { Ok(()) }
}

#[derive(Debug, thiserror::Error)]
#[error("the monthly AI budget is used up")]
pub struct BudgetExceeded;

pub struct UsageRecord<'a> {
    pub purpose: &'static str,
    pub model: &'a str,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub user_id: Option<Uuid>,
    pub run_id: Option<Uuid>,
}

/// Record one model call. Calls to `mock` cost nothing and are not recorded.
pub async fn record_usage(db: &PgPool, settings: &AiSettings, u: UsageRecord<'_>) {
    if u.model == "mock" {
        return;
    }
    let price = settings.prices.get(u.model);
    let cost = price.map_or(0.0, |p| (u.prompt_tokens as f64 * p.input_usd_per_mtok + u.completion_tokens as f64 * p.output_usd_per_mtok) / 1_000_000.0);
    let result = sqlx::query(
        "INSERT INTO model_usage (purpose, model, prompt_tokens, completion_tokens, cost_usd, priced, user_id, run_id)
         VALUES ($1, $2, $3, $4, $5::numeric, $6, $7, $8)",
    )
    .bind(u.purpose)
    .bind(u.model)
    .bind(u.prompt_tokens.min(i32::MAX as i64) as i32)
    .bind(u.completion_tokens.min(i32::MAX as i64) as i32)
    .bind(format!("{cost:.6}"))
    .bind(price.is_some())
    .bind(u.user_id)
    .bind(u.run_id)
    .execute(db)
    .await;
    if let Err(e) = result {
        tracing::error!(error = %e, "could not record model usage");
    }
}

/// Token counts from a genai usage value (`prompt_tokens`, `completion_tokens`).
pub fn tokens(usage: &Value) -> (i64, i64) {
    (usage["prompt_tokens"].as_i64().unwrap_or(0), usage["completion_tokens"].as_i64().unwrap_or(0))
}

// ---------------------------------------------------------------------------
// Management site
// ---------------------------------------------------------------------------

async fn get_settings(State(state): State<AppState>, _admin: AdminUser) -> AppResult<Json<AiSettings>> {
    Ok(Json(load(&state.db, &state.config).await?))
}

async fn put_settings(State(state): State<AppState>, AdminUser(admin): AdminUser, Json(s): Json<AiSettings>) -> AppResult<Json<AiSettings>> {
    s.validate().map_err(AppError::BadRequest)?;
    sqlx::query(
        "INSERT INTO app_settings (key, value, updated_by) VALUES ($1, $2, $3)
         ON CONFLICT (key) DO UPDATE SET value = $2, updated_by = $3, updated_at = now()",
    )
    .bind(KEY)
    .bind(serde_json::to_value(&s).map_err(anyhow::Error::from)?)
    .bind(admin.id)
    .execute(&state.db)
    .await?;
    Ok(Json(s))
}

#[derive(Serialize, sqlx::FromRow)]
struct UsageRow {
    model: String,
    purpose: String,
    calls: i64,
    prompt_tokens: i64,
    completion_tokens: i64,
    cost_usd: f64,
    unpriced_calls: i64,
}

async fn usage(State(state): State<AppState>, _staff: StaffUser) -> AppResult<Json<Value>> {
    let settings = load(&state.db, &state.config).await?;
    let start = month_start(academy_now());
    let rows = sqlx::query_as::<_, UsageRow>(
        "SELECT model, purpose, count(*) AS calls, COALESCE(SUM(prompt_tokens), 0)::int8 AS prompt_tokens,
                COALESCE(SUM(completion_tokens), 0)::int8 AS completion_tokens, COALESCE(SUM(cost_usd), 0)::float8 AS cost_usd,
                count(*) FILTER (WHERE NOT priced) AS unpriced_calls
         FROM model_usage WHERE occurred_at >= $1 GROUP BY model, purpose ORDER BY model, purpose",
    )
    .bind(start)
    .fetch_all(&state.db)
    .await?;
    // `+ 0.0` turns the empty sum (-0.0) into 0.0.
    let spent: f64 = rows.iter().map(|r| r.cost_usd).sum::<f64>() + 0.0;
    Ok(Json(json!({
        "month_start": start.date().to_string(),
        "spent_usd": spent,
        "monthly_budget_usd": settings.monthly_budget_usd,
        "on_budget_exceeded": settings.on_budget_exceeded,
        "blocked": settings.monthly_budget_usd.is_some_and(|l| spent >= l) && settings.on_budget_exceeded == BudgetPolicy::Block,
        "rows": rows,
    })))
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/settings", get(get_settings).put(put_settings)).route("/usage", get(usage))
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn month_starts_in_academy_time() {
        // 2026-10-31 20:00 UTC is already November 1 in Japan.
        assert_eq!(month_start(datetime!(2026-10-31 20:00 UTC).to_offset(ACADEMY_OFFSET)), datetime!(2026-11-01 00:00 +9));
    }

    #[test]
    fn rejects_bad_settings() {
        let mut s = AiSettings {
            chat_model: "gemini-3.8-flash".into(),
            memory_model: "gemini-3.8-flash".into(),
            embed_model: "gemini-embedding-2".into(),
            transcribe_model: "gemini-3.8-flash".into(),
            memory_enabled: true,
            monthly_budget_usd: Some(10.0),
            on_budget_exceeded: BudgetPolicy::Block,
            prices: BTreeMap::new(),
            conversation: ConversationSettings::default(),
        };
        assert!(s.validate().is_ok());
        s.chat_model = "rm -rf /".into();
        assert!(s.validate().is_err());
        s.chat_model = "mock".into();
        s.monthly_budget_usd = Some(-1.0);
        assert!(s.validate().is_err());
        s.monthly_budget_usd = None;
        s.conversation.max_steps = 0;
        assert!(s.validate().is_err());
        s.conversation.max_steps = 6;
        s.conversation.temperature = Some(3.0);
        assert!(s.validate().is_err());
    }

    #[test]
    fn old_stored_settings_get_conversation_defaults() {
        // A value saved before `conversation` existed, or with only some of its fields.
        let s: ConversationSettings = serde_json::from_value(json!({ "small_talk": "relaxed" })).unwrap();
        assert_eq!(s.small_talk, SmallTalk::Relaxed);
        assert_eq!(s.reply_length, ReplyLength::Natural);
        assert_eq!(s.max_output_tokens, 8192);
    }
}
