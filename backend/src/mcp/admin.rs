//! Management site for the MCP server: the switch, limits and budget, client keys, and call counts.

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{McpSettings, SETTINGS_KEY, sha256_hex};
use crate::auth::{AdminUser, StaffUser};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

#[derive(Serialize)]
struct SettingsView {
    settings: McpSettings,
    /// Remote access needs the read-only role (MCP_DATABASE_URL); without it `/mcp` answers 503.
    readonly_role_configured: bool,
    tools: Vec<String>,
}

async fn get_settings(State(state): State<AppState>, _admin: AdminUser) -> AppResult<Json<SettingsView>> {
    Ok(Json(SettingsView {
        settings: super::load_settings(&state.db).await?,
        readonly_role_configured: state.mcp.is_some(),
        tools: crate::skills::registry().mcp_tools().into_iter().map(String::from).collect(),
    }))
}

async fn put_settings(State(state): State<AppState>, AdminUser(admin): AdminUser, Json(s): Json<McpSettings>) -> AppResult<Json<McpSettings>> {
    s.validate().map_err(AppError::BadRequest)?;
    sqlx::query(
        "INSERT INTO app_settings (key, value, updated_at, updated_by) VALUES ($1, $2, now(), $3)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = now(), updated_by = EXCLUDED.updated_by",
    )
    .bind(SETTINGS_KEY)
    .bind(serde_json::to_value(&s).unwrap_or_default())
    .bind(admin.id)
    .execute(&state.db)
    .await?;
    Ok(Json(s))
}

#[derive(Serialize, sqlx::FromRow)]
struct ClientRow {
    id: Uuid,
    name: String,
    scopes: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    disabled_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    last_used_at: Option<OffsetDateTime>,
}

async fn clients(State(state): State<AppState>, _admin: AdminUser) -> AppResult<Json<Vec<ClientRow>>> {
    Ok(Json(
        sqlx::query_as::<_, ClientRow>("SELECT id, name, scopes, created_at, disabled_at, last_used_at FROM mcp_clients ORDER BY created_at DESC")
            .fetch_all(&state.db)
            .await?,
    ))
}

#[derive(Deserialize)]
struct NewClient {
    name: String,
    scopes: Option<Vec<String>>,
}

/// Create a client key. The key is returned once and only its hash is kept.
async fn create_client(State(state): State<AppState>, AdminUser(admin): AdminUser, Json(c): Json<NewClient>) -> AppResult<Json<Value>> {
    let name = c.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::BadRequest("name must be 1 to 80 characters".into()));
    }
    let scopes = c.scopes.unwrap_or_else(|| vec!["public:read".into(), "feedback:write".into()]);
    if scopes.is_empty() || scopes.iter().any(|s| s != "public:read" && s != "feedback:write") {
        return Err(AppError::BadRequest("scopes are public:read and feedback:write".into()));
    }
    let mut bytes = [0u8; 24];
    rand::fill(&mut bytes[..]);
    let key = format!("tmcp_{}", bytes.iter().map(|b| format!("{b:02x}")).collect::<String>());
    let id: Uuid = sqlx::query_scalar("INSERT INTO mcp_clients (name, key_hash, scopes, created_by) VALUES ($1, $2, $3, $4) RETURNING id")
        .bind(name)
        .bind(sha256_hex(&key))
        .bind(&scopes)
        .bind(admin.id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(json!({ "id": id, "key": key, "note": "Shown once. Send it as 'Authorization: Bearer <key>'." })))
}

async fn disable_client(State(state): State<AppState>, _admin: AdminUser, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let r = sqlx::query("UPDATE mcp_clients SET disabled_at = now() WHERE id = $1 AND disabled_at IS NULL").bind(id).execute(&state.db).await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("client"));
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Serialize, sqlx::FromRow)]
struct CallCount {
    tool: String,
    status: String,
    calls: i64,
}

/// Calls in the last seven days by tool and status, and feedback by status.
async fn activity(State(state): State<AppState>, _staff: StaffUser) -> AppResult<Json<Value>> {
    let calls = sqlx::query_as::<_, CallCount>(
        "SELECT tool, status, count(*) AS calls FROM mcp_calls WHERE occurred_at > now() - interval '7 days' GROUP BY tool, status ORDER BY tool, status",
    )
    .fetch_all(&state.db)
    .await?;
    let feedback: Vec<(String, i64)> = sqlx::query_as("SELECT status, count(*) FROM external_feedback GROUP BY status ORDER BY status").fetch_all(&state.db).await?;
    let spent = crate::settings::spent_this_month_public(&state.db, Some("mcp_embed")).await?;
    Ok(Json(json!({
        "calls": calls,
        "feedback": feedback.into_iter().map(|(s, n)| json!({ "status": s, "count": n })).collect::<Vec<_>>(),
        "mcp_embed_usd_this_month": spent,
    })))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/mcp/settings", get(get_settings).put(put_settings))
        .route("/mcp/clients", get(clients).post(create_client))
        .route("/mcp/clients/{id}/disable", post(disable_client))
        .route("/mcp/activity", get(activity))
}
