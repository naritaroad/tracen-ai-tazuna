//! Director's Office cases: a real person, shown as "Director Akikawa (staffed)", takes over.
//! While a case is in progress, the AI does not answer in that conversation.

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::auth::{CurrentUser, StaffUser};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub const REASONS: [&str; 5] = ["not_in_data", "exception_request", "complaint", "asked_for_director", "other"];

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TicketSummary {
    pub id: Uuid,
    pub ticket_no: String,
    pub requester_id: Uuid,
    pub requester_name: String,
    pub conversation_id: Option<Uuid>,
    pub reason: String,
    pub summary: String,
    pub origin: String,
    pub status: String,
    pub assignee_name: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub accepted_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub closed_at: Option<OffsetDateTime>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TicketEvent {
    pub id: Uuid,
    pub kind: String,
    pub body: String,
    /// Shown to the trainer as the persona; the real operator is kept for staff and audit.
    pub actor_name: Option<String>,
    pub actor_role: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

const TICKET_SELECT: &str = "SELECT t.id, t.ticket_no, t.requester_id, r.display_name AS requester_name, t.conversation_id, t.reason, t.summary,
            t.origin, t.status, a.display_name AS assignee_name, t.created_at, t.accepted_at, t.closed_at
     FROM director_tickets t JOIN users r ON r.id = t.requester_id LEFT JOIN users a ON a.id = t.assignee_id";

pub struct NewTicket<'a> {
    pub requester: &'a CurrentUser,
    pub conversation_id: Option<Uuid>,
    pub reason: &'a str,
    pub summary: &'a str,
    pub origin: &'static str,
    pub idempotency_key: String,
}

async fn fetch_ticket(db: &PgPool, id: Uuid) -> sqlx::Result<Option<TicketSummary>> {
    // TICKET_SELECT is a constant; sqlx only accepts static SQL text.
    sqlx::query_as::<_, TicketSummary>(sqlx::AssertSqlSafe(format!("{TICKET_SELECT} WHERE t.id = $1"))).bind(id).fetch_optional(db).await
}

/// Create a case, or return the existing one for the same idempotency key. `bool` = newly created.
pub async fn create_ticket(db: &PgPool, t: &NewTicket<'_>) -> sqlx::Result<(TicketSummary, bool)> {
    let mut tx = db.begin().await?;
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO director_tickets (requester_id, conversation_id, reason, summary, origin, idempotency_key)
         VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (idempotency_key) DO NOTHING RETURNING id",
    )
    .bind(t.requester.id)
    .bind(t.conversation_id)
    .bind(t.reason)
    .bind(t.summary)
    .bind(t.origin)
    .bind(&t.idempotency_key)
    .fetch_optional(&mut *tx)
    .await?;
    let (id, created) = match inserted {
        Some(id) => {
            sqlx::query("INSERT INTO ticket_events (ticket_id, actor_user_id, kind, body) VALUES ($1, $2, 'created', $3)")
                .bind(id)
                .bind(t.requester.id)
                .bind(t.summary)
                .execute(&mut *tx)
                .await?;
            (id, true)
        }
        None => (
            sqlx::query_scalar("SELECT id FROM director_tickets WHERE idempotency_key = $1").bind(&t.idempotency_key).fetch_one(&mut *tx).await?,
            false,
        ),
    };
    tx.commit().await?;
    let ticket = fetch_ticket(db, id).await?.ok_or(sqlx::Error::RowNotFound)?;
    Ok((ticket, created))
}

/// The case that currently has a person in charge of this conversation, if any.
pub async fn active_for_conversation(db: &PgPool, conversation_id: Uuid) -> sqlx::Result<Option<TicketSummary>> {
    sqlx::query_as::<_, TicketSummary>(sqlx::AssertSqlSafe(format!(
        "{TICKET_SELECT} WHERE t.conversation_id = $1 AND t.status = 'in_progress' ORDER BY t.accepted_at DESC LIMIT 1"
    )))
    .bind(conversation_id)
    .fetch_optional(db)
    .await
}

pub async fn record_trainer_message(db: &PgPool, ticket_id: Uuid, user_id: Uuid, body: &str) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO ticket_events (ticket_id, actor_user_id, kind, body) VALUES ($1, $2, 'trainer_message', $3)")
        .bind(ticket_id)
        .bind(user_id)
        .bind(body)
        .execute(db)
        .await?;
    Ok(())
}

async fn events_for(db: &PgPool, ticket_id: Uuid) -> sqlx::Result<Vec<TicketEvent>> {
    sqlx::query_as::<_, TicketEvent>(
        "SELECT e.id, e.kind, e.body, u.display_name AS actor_name, u.role AS actor_role, e.created_at
         FROM ticket_events e LEFT JOIN users u ON u.id = e.actor_user_id WHERE e.ticket_id = $1 ORDER BY e.created_at",
    )
    .bind(ticket_id)
    .fetch_all(db)
    .await
}

// ---------------------------------------------------------------------------
// Trainer side: my cases, and the manual form (no AI)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct TicketForm {
    reason: String,
    summary: String,
    /// Client-generated id, so a double click does not open two cases.
    request_id: Uuid,
}

async fn create_from_form(State(state): State<AppState>, user: CurrentUser, Json(form): Json<TicketForm>) -> AppResult<Json<TicketSummary>> {
    if !REASONS.contains(&form.reason.as_str()) {
        return Err(AppError::BadRequest(format!("reason must be one of {REASONS:?}")));
    }
    let summary = form.summary.trim();
    if summary.is_empty() || summary.chars().count() > 2000 {
        return Err(AppError::BadRequest("summary must be 1 to 2000 characters".into()));
    }
    let request = NewTicket {
        requester: &user,
        conversation_id: None,
        reason: &form.reason,
        summary,
        origin: "form",
        idempotency_key: format!("form:{}:{}", user.id, form.request_id),
    };
    Ok(Json(create_ticket(&state.db, &request).await?.0))
}

async fn my_tickets(State(state): State<AppState>, user: CurrentUser) -> AppResult<Json<Vec<TicketSummary>>> {
    let rows = sqlx::query_as::<_, TicketSummary>(sqlx::AssertSqlSafe(format!(
        "{TICKET_SELECT} WHERE t.requester_id = $1 ORDER BY t.created_at DESC LIMIT 100"
    )))
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Serialize)]
struct TicketDetail {
    ticket: TicketSummary,
    events: Vec<TicketEvent>,
}

async fn my_ticket(State(state): State<AppState>, user: CurrentUser, Path(id): Path<Uuid>) -> AppResult<Json<TicketDetail>> {
    let ticket = fetch_ticket(&state.db, id).await?.filter(|t| t.requester_id == user.id).ok_or(AppError::NotFound("case"))?;
    let events = events_for(&state.db, id).await?;
    Ok(Json(TicketDetail { ticket, events }))
}

pub fn trainer_routes() -> Router<AppState> {
    Router::new().route("/tickets", get(my_tickets).post(create_from_form)).route("/tickets/{id}", get(my_ticket))
}

// ---------------------------------------------------------------------------
// Management site (staff, no AI)
// ---------------------------------------------------------------------------

async fn inbox(State(state): State<AppState>, _staff: StaffUser) -> AppResult<Json<Vec<TicketSummary>>> {
    let rows = sqlx::query_as::<_, TicketSummary>(sqlx::AssertSqlSafe(format!(
        "{TICKET_SELECT} ORDER BY (t.status IN ('waiting', 'in_progress')) DESC, t.created_at DESC LIMIT 200"
    )))
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

async fn staff_ticket(State(state): State<AppState>, _staff: StaffUser, Path(id): Path<Uuid>) -> AppResult<Json<TicketDetail>> {
    let ticket = fetch_ticket(&state.db, id).await?.ok_or(AppError::NotFound("case"))?;
    let events = events_for(&state.db, id).await?;
    Ok(Json(TicketDetail { ticket, events }))
}

async fn notice(db: &PgPool, conversation_id: Option<Uuid>, text: &str) -> sqlx::Result<()> {
    if let Some(cid) = conversation_id {
        sqlx::query("INSERT INTO messages (conversation_id, role, content) VALUES ($1, 'notice', $2)").bind(cid).bind(text).execute(db).await?;
        sqlx::query("UPDATE conversations SET updated_at = now() WHERE id = $1").bind(cid).execute(db).await?;
    }
    Ok(())
}

async fn accept(State(state): State<AppState>, StaffUser(staff): StaffUser, Path(id): Path<Uuid>) -> AppResult<Json<TicketSummary>> {
    let mut tx = state.db.begin().await?;
    let conversation: Option<Option<Uuid>> = sqlx::query_scalar(
        "UPDATE director_tickets SET status = 'in_progress', assignee_id = $2, accepted_at = now()
         WHERE id = $1 AND status = 'waiting' RETURNING conversation_id",
    )
    .bind(id)
    .bind(staff.id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(conversation) = conversation else {
        return Err(AppError::Conflict("only a waiting case can be accepted".into()));
    };
    sqlx::query("INSERT INTO ticket_events (ticket_id, actor_user_id, kind) VALUES ($1, $2, 'accepted')").bind(id).bind(staff.id).execute(&mut *tx).await?;
    if let Some(cid) = conversation {
        sqlx::query("UPDATE conversations SET status = 'handoff', updated_at = now() WHERE id = $1").bind(cid).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    notice(&state.db, conversation, "director.accepted").await?;
    Ok(Json(fetch_ticket(&state.db, id).await?.ok_or(AppError::NotFound("case"))?))
}

#[derive(Deserialize)]
struct ReplyBody {
    body: String,
}

async fn reply(State(state): State<AppState>, StaffUser(staff): StaffUser, Path(id): Path<Uuid>, Json(req): Json<ReplyBody>) -> AppResult<Json<TicketSummary>> {
    let body = req.body.trim();
    if body.is_empty() || body.chars().count() > 4000 {
        return Err(AppError::BadRequest("reply must be 1 to 4000 characters".into()));
    }
    let ticket = fetch_ticket(&state.db, id).await?.ok_or(AppError::NotFound("case"))?;
    if ticket.status != "in_progress" {
        return Err(AppError::Conflict("accept the case before replying".into()));
    }
    let mut tx = state.db.begin().await?;
    sqlx::query("INSERT INTO ticket_events (ticket_id, actor_user_id, kind, body) VALUES ($1, $2, 'reply', $3)")
        .bind(id)
        .bind(staff.id)
        .bind(body)
        .execute(&mut *tx)
        .await?;
    if let Some(cid) = ticket.conversation_id {
        sqlx::query("INSERT INTO messages (conversation_id, role, content, author_user_id) VALUES ($1, 'director', $2, $3)")
            .bind(cid)
            .bind(body)
            .bind(staff.id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE conversations SET updated_at = now() WHERE id = $1").bind(cid).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(Json(fetch_ticket(&state.db, id).await?.ok_or(AppError::NotFound("case"))?))
}

/// Resolve, or hand the conversation back to the AI without resolving.
async fn close(state: &AppState, staff: &CurrentUser, id: Uuid, status: &'static str) -> AppResult<Json<TicketSummary>> {
    let mut tx = state.db.begin().await?;
    let conversation: Option<Option<Uuid>> = sqlx::query_scalar(
        "UPDATE director_tickets SET status = $2, closed_at = now(), assignee_id = COALESCE(assignee_id, $3)
         WHERE id = $1 AND status IN ('waiting', 'in_progress') RETURNING conversation_id",
    )
    .bind(id)
    .bind(status)
    .bind(staff.id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(conversation) = conversation else {
        return Err(AppError::Conflict("the case is already closed".into()));
    };
    sqlx::query("INSERT INTO ticket_events (ticket_id, actor_user_id, kind) VALUES ($1, $2, $3)").bind(id).bind(staff.id).bind(status).execute(&mut *tx).await?;
    if let Some(cid) = conversation {
        sqlx::query("UPDATE conversations SET status = 'active', updated_at = now() WHERE id = $1 AND status = 'handoff'").bind(cid).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    notice(&state.db, conversation, if status == "resolved" { "director.resolved" } else { "director.returned" }).await?;
    Ok(Json(fetch_ticket(&state.db, id).await?.ok_or(AppError::NotFound("case"))?))
}

async fn resolve(State(state): State<AppState>, StaffUser(staff): StaffUser, Path(id): Path<Uuid>) -> AppResult<Json<TicketSummary>> {
    close(&state, &staff, id, "resolved").await
}

async fn hand_back(State(state): State<AppState>, StaffUser(staff): StaffUser, Path(id): Path<Uuid>) -> AppResult<Json<TicketSummary>> {
    close(&state, &staff, id, "returned").await
}

pub fn staff_routes() -> Router<AppState> {
    Router::new()
        .route("/tickets", get(inbox))
        .route("/tickets/{id}", get(staff_ticket))
        .route("/tickets/{id}/accept", post(accept))
        .route("/tickets/{id}/reply", post(reply))
        .route("/tickets/{id}/resolve", post(resolve))
        .route("/tickets/{id}/return", post(hand_back))
}
