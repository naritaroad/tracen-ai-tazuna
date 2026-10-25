//! Management site endpoints (no AI): demo catalog upkeep, accounts and run audit.

use axum::extract::{Path, State};
use axum::routing::{get, patch, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::auth::{AdminUser, StaffUser};
use crate::catalog;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Serialize)]
struct ServiceAdminView {
    id: String,
    name: String,
    name_ja: String,
    name_en: String,
    category: String,
    unit_minutes: i32,
    capacity: i32,
    fee_jpy: i32,
    available: bool,
    location: String,
    notes: String,
    source_type: String,
}

async fn services(State(state): State<AppState>, _staff: StaffUser) -> AppResult<Json<Vec<ServiceAdminView>>> {
    let rows = catalog::list_services(&state.db).await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| ServiceAdminView {
                id: r.id,
                name: r.name,
                name_ja: r.name_ja,
                name_en: r.name_en,
                category: r.category,
                unit_minutes: r.unit_minutes,
                capacity: r.capacity,
                fee_jpy: r.fee_jpy,
                available: r.available,
                location: r.location,
                notes: r.notes,
                source_type: r.source_type,
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
struct ServicePatch {
    available: Option<bool>,
    capacity: Option<i32>,
    fee_jpy: Option<i32>,
    notes: Option<String>,
}

async fn update_service(State(state): State<AppState>, _staff: StaffUser, Path(id): Path<String>, Json(p): Json<ServicePatch>) -> AppResult<Json<Value>> {
    if p.capacity.is_some_and(|c| !(1..=500).contains(&c)) || p.fee_jpy.is_some_and(|f| !(0..=1_000_000).contains(&f)) {
        return Err(AppError::BadRequest("capacity must be 1–500 and fee_jpy 0–1,000,000".into()));
    }
    let done = sqlx::query(
        "UPDATE training_services SET available = COALESCE($2, available), capacity = COALESCE($3, capacity),
                fee_jpy = COALESCE($4, fee_jpy), notes = COALESCE($5, notes) WHERE id = $1",
    )
    .bind(&id)
    .bind(p.available)
    .bind(p.capacity)
    .bind(p.fee_jpy)
    .bind(p.notes.map(|n| n.trim().chars().take(200).collect::<String>()))
    .execute(&state.db)
    .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound("service"));
    }
    Ok(Json(json!({ "updated": true, "note": "With CATALOG_SEED_MODE=reset the CSV replaces this change at the next start." })))
}

#[derive(Serialize)]
struct InfoAdminView {
    topic: String,
    title: String,
    content: String,
    content_ja: String,
    content_en: String,
    source_type: String,
    source_ref: String,
    updated_on: String,
}

async fn info(State(state): State<AppState>, _staff: StaffUser) -> AppResult<Json<Vec<InfoAdminView>>> {
    let rows = catalog::list_info(&state.db).await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| InfoAdminView {
                topic: r.topic,
                title: r.title,
                content: r.content,
                content_ja: r.content_ja,
                content_en: r.content_en,
                source_type: r.source_type,
                source_ref: r.source_ref,
                updated_on: r.updated_on.to_string(),
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
struct InfoUpdate {
    content: String,
    content_ja: String,
    content_en: String,
}

async fn update_info(State(state): State<AppState>, _staff: StaffUser, Path(topic): Path<String>, Json(u): Json<InfoUpdate>) -> AppResult<Json<Value>> {
    for text in [&u.content, &u.content_ja, &u.content_en] {
        if text.trim().is_empty() || text.chars().count() > 2000 {
            return Err(AppError::BadRequest("each content must be 1 to 2000 characters".into()));
        }
    }
    let done = sqlx::query("UPDATE academy_info SET content = $2, content_ja = $3, content_en = $4, updated_on = CURRENT_DATE WHERE topic = $1")
        .bind(&topic)
        .bind(u.content.trim())
        .bind(u.content_ja.trim())
        .bind(u.content_en.trim())
        .execute(&state.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(AppError::NotFound("topic"));
    }
    Ok(Json(json!({ "updated": true })))
}

#[derive(Serialize, sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    username: String,
    display_name: String,
    role: String,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    disabled: bool,
}

async fn users(State(state): State<AppState>, _admin: AdminUser) -> AppResult<Json<Vec<UserRow>>> {
    let rows = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, display_name, role, created_at, disabled_at IS NOT NULL AS disabled FROM users ORDER BY created_at",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Serialize, sqlx::FromRow)]
struct RunAudit {
    id: Uuid,
    conversation_id: Uuid,
    trainer: String,
    model: String,
    locale: String,
    status: String,
    steps: i32,
    error_code: Option<String>,
    tools: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    started_at: OffsetDateTime,
}

async fn runs(State(state): State<AppState>, _staff: StaffUser) -> AppResult<Json<Vec<RunAudit>>> {
    let rows = sqlx::query_as::<_, RunAudit>(
        "SELECT r.id, r.conversation_id, u.display_name AS trainer, r.model, r.locale, r.status, r.steps, r.error_code,
                COALESCE(array_agg(t.tool_name ORDER BY t.created_at) FILTER (WHERE t.id IS NOT NULL), '{}') AS tools, r.started_at
         FROM agent_runs r JOIN conversations c ON c.id = r.conversation_id JOIN users u ON u.id = c.user_id
         LEFT JOIN tool_calls t ON t.run_id = r.id
         GROUP BY r.id, u.display_name ORDER BY r.started_at DESC LIMIT 100",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/catalog/services", get(services))
        .route("/catalog/services/{id}", patch(update_service))
        .route("/catalog/info", get(info))
        .route("/catalog/info/{topic}", put(update_info))
        .route("/users", get(users))
        .route("/runs", get(runs))
}
