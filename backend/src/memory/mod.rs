//! What Tazuna remembers about a trainer (questionnaire Q08 B). Four layers: recent messages,
//! a summary of older messages, preferences and facts, and past events found again by meaning.
//! The trainer can see, change and delete all of it. Memory shapes how she addresses and helps
//! the trainer; it is never a source of academy facts.

pub mod curate;

use axum::extract::{Path, State};
use axum::routing::{get, patch};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use crate::llm::{Ai, vector_literal};
use crate::locale::Locale;
use crate::settings::{self, AiSettings, UsageRecord};
use crate::state::AppState;

/// Past events closer than this cosine distance count as related to the new question.
const RECALL_MAX_DISTANCE: f64 = 0.6;
const RECALL_LIMIT: i64 = 5;

#[derive(Debug, Default, Serialize, Deserialize, sqlx::FromRow)]
pub struct Profile {
    pub preferred_name: Option<String>,
    pub reply_locale: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MemoryItem {
    pub id: Uuid,
    pub kind: String,
    pub content: String,
    pub source: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

pub async fn load_profile(db: &PgPool, user_id: Uuid) -> sqlx::Result<Profile> {
    Ok(sqlx::query_as::<_, Profile>("SELECT preferred_name, reply_locale FROM trainer_profiles WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(db)
        .await?
        .unwrap_or_default())
}

pub async fn active_items(db: &PgPool, user_id: Uuid) -> sqlx::Result<Vec<MemoryItem>> {
    sqlx::query_as::<_, MemoryItem>(
        "SELECT id, kind, content, source, created_at, updated_at FROM memory_items
         WHERE user_id = $1 AND status = 'active' ORDER BY created_at LIMIT 50",
    )
    .bind(user_id)
    .fetch_all(db)
    .await
}

#[derive(sqlx::FromRow)]
struct Recalled {
    content: String,
    created_at: OffsetDateTime,
}

/// Past events related to `question`, best first. Failures are logged and give nothing:
/// a reply without recall is better than no reply.
pub async fn recall(db: &PgPool, ai: &Ai, settings: &AiSettings, user_id: Uuid, question: &str) -> Vec<String> {
    if !settings.memory_enabled || question.trim().is_empty() || settings::check_budget(db, settings).await.is_err() {
        return Vec::new();
    }
    let embedding = match ai.embed(&settings.embed_model, question).await {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(error = ?e, "recall embedding failed");
            return Vec::new();
        }
    };
    settings::record_usage(
        db,
        settings,
        UsageRecord { purpose: "embed", model: &settings.embed_model, prompt_tokens: embedding.prompt_tokens, completion_tokens: 0, user_id: Some(user_id), run_id: None },
    )
    .await;
    let rows = sqlx::query_as::<_, Recalled>(
        "SELECT content, created_at FROM memory_events
         WHERE user_id = $1 AND embed_model = $2 AND embedding IS NOT NULL AND (embedding <=> $3::vector) < $4
         ORDER BY embedding <=> $3::vector LIMIT $5",
    )
    .bind(user_id)
    .bind(&settings.embed_model)
    .bind(vector_literal(&embedding.vector))
    .bind(RECALL_MAX_DISTANCE)
    .bind(RECALL_LIMIT)
    .fetch_all(db)
    .await;
    match rows {
        Ok(rows) => rows.into_iter().map(|r| format!("{} ({})", r.content, r.created_at.date())).collect(),
        Err(e) => {
            tracing::warn!(error = %e, "recall query failed");
            Vec::new()
        }
    }
}

/// The memory block of the system prompt.
pub async fn prompt_context(
    db: &PgPool,
    ai: &Ai,
    settings: &AiSettings,
    user: &CurrentUser,
    conversation_id: Uuid,
    question: &str,
) -> sqlx::Result<String> {
    let profile = load_profile(db, user.id).await?;
    let items = active_items(db, user.id).await?;
    let mut text = String::from(
        "TRAINER PROFILE (from the trainer and from earlier conversations; use it only for how you address and help them, \
         never as academy facts)\n",
    );
    text.push_str(&format!("- account name: {}\n", user.display_name));
    if let Some(name) = profile.preferred_name.as_deref().filter(|n| !n.is_empty()) {
        text.push_str(&format!("- call them: {name}\n"));
    }
    for item in items {
        text.push_str(&format!("- {}: {}\n", item.kind, item.content));
    }
    let events = recall(db, ai, settings, user.id, question).await;
    if !events.is_empty() {
        text.push_str("\nRELATED PAST EVENTS (may be out of date; check current data with tools)\n");
        for e in events {
            text.push_str(&format!("- {e}\n"));
        }
    }
    let summary: Option<String> = sqlx::query_scalar("SELECT summary FROM conversations WHERE id = $1 AND summary_message_count > 0")
        .bind(conversation_id)
        .fetch_optional(db)
        .await?
        .flatten();
    if let Some(summary) = summary {
        text.push_str(&format!("\nEARLIER IN THIS CONVERSATION (summary of messages no longer shown)\n{summary}\n"));
    }
    Ok(text)
}

/// The trainer's chosen reply language, if they set one.
pub async fn reply_locale(db: &PgPool, user_id: Uuid) -> sqlx::Result<Option<Locale>> {
    Ok(load_profile(db, user_id).await?.reply_locale.as_deref().and_then(Locale::parse))
}

async fn get_profile(State(state): State<AppState>, user: CurrentUser) -> AppResult<Json<Profile>> {
    Ok(Json(load_profile(&state.db, user.id).await?))
}

async fn put_profile(State(state): State<AppState>, user: CurrentUser, Json(p): Json<Profile>) -> AppResult<Json<Profile>> {
    let name = p.preferred_name.map(|n| n.trim().chars().take(40).collect::<String>()).filter(|n| !n.is_empty());
    let locale = match p.reply_locale.as_deref().filter(|l| !l.is_empty()) {
        None => None,
        Some(l) => Some(Locale::parse(l).ok_or_else(|| AppError::BadRequest("reply_locale must be zh-TW, ja or en-GB".into()))?.code().to_string()),
    };
    sqlx::query(
        "INSERT INTO trainer_profiles (user_id, preferred_name, reply_locale) VALUES ($1, $2, $3)
         ON CONFLICT (user_id) DO UPDATE SET preferred_name = $2, reply_locale = $3, updated_at = now()",
    )
    .bind(user.id)
    .bind(&name)
    .bind(&locale)
    .execute(&state.db)
    .await?;
    Ok(Json(Profile { preferred_name: name, reply_locale: locale }))
}

async fn list_items(State(state): State<AppState>, user: CurrentUser) -> AppResult<Json<Vec<MemoryItem>>> {
    Ok(Json(active_items(&state.db, user.id).await?))
}

#[derive(Deserialize)]
struct ItemBody {
    kind: Option<String>,
    content: String,
}

fn check_item(body: &ItemBody) -> AppResult<(String, String)> {
    let kind = body.kind.clone().unwrap_or_else(|| "preference".into());
    if !["preference", "fact"].contains(&kind.as_str()) {
        return Err(AppError::BadRequest("kind must be preference or fact".into()));
    }
    let content = body.content.trim().to_string();
    if content.is_empty() || content.chars().count() > 500 {
        return Err(AppError::BadRequest("content must be 1 to 500 characters".into()));
    }
    Ok((kind, content))
}

async fn add_item(State(state): State<AppState>, user: CurrentUser, Json(body): Json<ItemBody>) -> AppResult<Json<MemoryItem>> {
    let (kind, content) = check_item(&body)?;
    let item = sqlx::query_as::<_, MemoryItem>(
        "INSERT INTO memory_items (user_id, kind, content, source) VALUES ($1, $2, $3, 'trainer')
         RETURNING id, kind, content, source, created_at, updated_at",
    )
    .bind(user.id)
    .bind(kind)
    .bind(content)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(item))
}

async fn update_item(State(state): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>, Json(body): Json<ItemBody>) -> AppResult<Json<MemoryItem>> {
    let (kind, content) = check_item(&body)?;
    sqlx::query_as::<_, MemoryItem>(
        "UPDATE memory_items SET kind = $3, content = $4, updated_at = now()
         WHERE id = $1 AND user_id = $2 AND status = 'active'
         RETURNING id, kind, content, source, created_at, updated_at",
    )
    .bind(id)
    .bind(user.id)
    .bind(kind)
    .bind(content)
    .fetch_optional(&state.db)
    .await?
    .map(Json)
    .ok_or(AppError::NotFound("memory item"))
}

/// Forget: the text is overwritten, not only hidden.
async fn delete_item(State(state): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>) -> AppResult<Json<serde_json::Value>> {
    let done = sqlx::query(
        "UPDATE memory_items SET status = 'deleted', content = '[deleted]', embedding = NULL, updated_at = now()
         WHERE id = $1 AND user_id = $2 AND status = 'active'",
    )
    .bind(id)
    .bind(user.id)
    .execute(&state.db)
    .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound("memory item"));
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct EventRow {
    id: Uuid,
    content: String,
    conversation_id: Option<Uuid>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

async fn list_events(State(state): State<AppState>, user: CurrentUser) -> AppResult<Json<Vec<EventRow>>> {
    let rows = sqlx::query_as::<_, EventRow>(
        "SELECT id, content, conversation_id, created_at FROM memory_events WHERE user_id = $1 ORDER BY created_at DESC LIMIT 200",
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

/// Forget an event: the row and its vector are removed.
async fn delete_event(State(state): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>) -> AppResult<Json<serde_json::Value>> {
    let done = sqlx::query("DELETE FROM memory_events WHERE id = $1 AND user_id = $2").bind(id).bind(user.id).execute(&state.db).await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound("memory event"));
    }
    Ok(Json(serde_json::json!({ "deleted": true })))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/profile", get(get_profile).put(put_profile))
        .route("/memory", get(list_items).post(add_item))
        .route("/memory/{id}", patch(update_item).delete(delete_item))
        .route("/memory/events", get(list_events))
        .route("/memory/events/{id}", axum::routing::delete(delete_event))
}
