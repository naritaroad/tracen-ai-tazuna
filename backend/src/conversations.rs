//! Trainer conversations. Posting a message answers with an SSE stream of run events.

use std::convert::Infallible;

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use time::OffsetDateTime;
use futures::StreamExt;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;

use crate::agent::{self, RunInput, StreamEvent};
use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use crate::handoff;
use crate::locale::Locale;
use crate::memory;
use crate::settings;
use crate::state::AppState;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ConversationRow {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    /// Pinned to the top of the sidebar.
    pub pinned: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

const CONVERSATION_COLUMNS: &str = "id, title, status, pinned_at IS NOT NULL AS pinned, created_at, updated_at";

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MessageRow {
    pub id: Uuid,
    pub role: String,
    pub content: String,
    pub run_id: Option<Uuid>,
    /// Display name of a Director's Office reply author (the persona is shown by the UI).
    pub author_name: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ToolCallRow {
    pub run_id: Uuid,
    pub step: i32,
    pub call_id: String,
    pub tool_name: String,
    pub arguments: Value,
    pub result: Value,
    pub status: String,
    pub duration_ms: i32,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RunRow {
    pub id: Uuid,
    pub status: String,
    pub model: String,
    pub locale: String,
    pub steps: i32,
    pub error_code: Option<String>,
}

async fn owned(state: &AppState, user: &CurrentUser, id: Uuid) -> AppResult<ConversationRow> {
    sqlx::query_as::<_, ConversationRow>(sqlx::AssertSqlSafe(format!("SELECT {CONVERSATION_COLUMNS} FROM conversations WHERE id = $1 AND user_id = $2")))
        .bind(id)
        .bind(user.id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound("conversation"))
}

async fn list(State(state): State<AppState>, user: CurrentUser) -> AppResult<Json<Vec<ConversationRow>>> {
    // Pinned first (newest pin on top), so they are never cut off by the limit.
    let rows = sqlx::query_as::<_, ConversationRow>(sqlx::AssertSqlSafe(format!(
        "SELECT {CONVERSATION_COLUMNS} FROM conversations WHERE user_id = $1 ORDER BY pinned_at DESC NULLS LAST, updated_at DESC LIMIT 50"
    )))
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

async fn create(State(state): State<AppState>, user: CurrentUser) -> AppResult<Json<ConversationRow>> {
    let row = sqlx::query_as::<_, ConversationRow>(sqlx::AssertSqlSafe(format!("INSERT INTO conversations (user_id) VALUES ($1) RETURNING {CONVERSATION_COLUMNS}")))
    .bind(user.id)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(row))
}

#[derive(Deserialize)]
struct ConversationPatch {
    pinned: bool,
}

/// Pin or unpin. Pinning does not touch `updated_at`, so the conversation keeps its place in time.
async fn patch_conversation(State(state): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>, Json(req): Json<ConversationPatch>) -> AppResult<Json<ConversationRow>> {
    sqlx::query_as::<_, ConversationRow>(sqlx::AssertSqlSafe(format!(
        "UPDATE conversations SET pinned_at = CASE WHEN $3 THEN coalesce(pinned_at, now()) END
         WHERE id = $1 AND user_id = $2 RETURNING {CONVERSATION_COLUMNS}"
    )))
    .bind(id)
    .bind(user.id)
    .bind(req.pinned)
    .fetch_optional(&state.db)
    .await?
    .map(Json)
    .ok_or(AppError::NotFound("conversation"))
}

#[derive(Serialize)]
struct ConversationDetail {
    conversation: ConversationRow,
    messages: Vec<MessageRow>,
    runs: Vec<RunRow>,
    tool_calls: Vec<ToolCallRow>,
    active_ticket: Option<handoff::TicketSummary>,
}

async fn detail(State(state): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>) -> AppResult<Json<ConversationDetail>> {
    let conversation = owned(&state, &user, id).await?;
    let messages = sqlx::query_as::<_, MessageRow>(
        "SELECT m.id, m.role, m.content, m.run_id, CASE WHEN m.role = 'director' THEN u.display_name END AS author_name, m.created_at
         FROM messages m LEFT JOIN users u ON u.id = m.author_user_id WHERE m.conversation_id = $1 ORDER BY m.created_at",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let runs = sqlx::query_as::<_, RunRow>("SELECT id, status, model, locale, steps, error_code FROM agent_runs WHERE conversation_id = $1 ORDER BY started_at")
        .bind(id)
        .fetch_all(&state.db)
        .await?;
    let tool_calls = sqlx::query_as::<_, ToolCallRow>(
        "SELECT t.run_id, t.step, t.call_id, t.tool_name, t.arguments, t.result, t.status, t.duration_ms
         FROM tool_calls t JOIN agent_runs r ON r.id = t.run_id WHERE r.conversation_id = $1 ORDER BY t.created_at",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let active_ticket = handoff::active_for_conversation(&state.db, id).await?;
    Ok(Json(ConversationDetail { conversation, messages, runs, tool_calls, active_ticket }))
}

#[derive(Deserialize)]
struct PostMessage {
    content: String,
    /// The interface language, used when the text alone does not show a language.
    locale: Option<String>,
}

fn sse_event(seq: u64, e: StreamEvent) -> Event {
    let payload = json!({ "v": 1, "seq": seq, "type": e.kind, "data": e.data });
    Event::default().event(e.kind).data(payload.to_string())
}

async fn post_message(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(req): Json<PostMessage>,
) -> AppResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let conversation = owned(&state, &user, id).await?;
    let content = req.content.trim().to_string();
    if content.is_empty() || content.chars().count() > 2000 {
        return Err(AppError::BadRequest("message must be 1 to 2000 characters".into()));
    }
    if conversation.status == "closed" {
        return Err(AppError::Conflict("this conversation is closed".into()));
    }

    let message_id: Uuid = sqlx::query_scalar("INSERT INTO messages (conversation_id, role, content, author_user_id) VALUES ($1, 'user', $2, $3) RETURNING id")
        .bind(id)
        .bind(&content)
        .bind(user.id)
        .fetch_one(&state.db)
        .await?;
    let title: String = content.chars().take(40).collect();
    sqlx::query("UPDATE conversations SET updated_at = now(), title = CASE WHEN title = '' THEN $2 ELSE title END WHERE id = $1")
        .bind(id)
        .bind(&title)
        .execute(&state.db)
        .await?;

    // Reply language: the trainer's setting, else the message text, else the interface language.
    let ui_locale = req.locale.as_deref().and_then(Locale::parse).or(user.preferred_locale.as_deref().and_then(Locale::parse)).unwrap_or(Locale::ZhTw);
    let locale = match memory::reply_locale(&state.db, user.id).await? {
        Some(l) => l,
        None => Locale::detect(&content, ui_locale),
    };

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<StreamEvent>();
    let _ = tx.send(StreamEvent { kind: "message.accepted", data: json!({ "message_id": message_id, "conversation_id": id }) });

    // A person has the case: the message goes to them, not to the AI.
    if let Some(ticket) = handoff::active_for_conversation(&state.db, id).await? {
        handoff::record_trainer_message(&state.db, ticket.id, user.id, &content).await?;
        let _ = tx.send(StreamEvent { kind: "handoff.active", data: json!({ "ticket_no": ticket.ticket_no }) });
        drop(tx);
    } else {
        // Settings are read per message, so changes on the management site apply at once.
        let ai_settings = settings::load(&state.db, &state.config).await?;
        let task_state = state.clone();
        tokio::spawn(async move {
            let input = RunInput {
                db: &task_state.db,
                ai: &task_state.ai,
                settings: &ai_settings,
                user: &user,
                conversation_id: id,
                locale,
            };
            match agent::run(input, &tx).await {
                // The stream is finished; memory is curated in the background.
                Ok(result) if result.message_id.is_some() => {
                    drop(tx);
                    memory::curate::after_run(task_state.db.clone(), (*task_state.ai).clone(), ai_settings, user.id, id, result.run_id).await;
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::error!(error = ?e, "agent run could not be recorded");
                    let _ = tx.send(StreamEvent { kind: "run.failed", data: json!({ "code": "internal", "message": "internal error" }) });
                }
            }
        });
    }

    let stream = UnboundedReceiverStream::new(rx).enumerate().map(|(i, e)| Ok(sse_event(i as u64 + 1, e)));
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/conversations", get(list).post(create))
        .route("/conversations/{id}", get(detail).patch(patch_conversation))
        .route("/conversations/{id}/messages", post(post_message))
}
