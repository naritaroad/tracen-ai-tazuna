//! Accounts and cookie sessions. Roles: trainer, staff (Director's Office operators), admin.

use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub const SESSION_COOKIE: &str = "tracen_session";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Trainer,
    Staff,
    Admin,
}

impl TryFrom<String> for Role {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "trainer" => Ok(Self::Trainer),
            "staff" => Ok(Self::Staff),
            "admin" => Ok(Self::Admin),
            other => Err(format!("unknown role {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CurrentUser {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    #[sqlx(try_from = "String")]
    pub role: Role,
    pub preferred_locale: Option<String>,
    /// Milliseconds of the profile picture's last change (a cache key for `/me/avatar`); none without one.
    pub avatar_version: Option<i64>,
    /// What the trainer asked to be called ("My profile"); the account button shows it over the account name.
    pub preferred_name: Option<String>,
}

/// Columns of `CurrentUser` for a query over `users u`.
pub const USER_COLUMNS: &str = "u.id, u.username, u.display_name, u.role, u.preferred_locale, \
     (SELECT (extract(epoch FROM a.updated_at) * 1000)::bigint FROM user_avatars a WHERE a.user_id = u.id) AS avatar_version, \
     (SELECT p.preferred_name FROM trainer_profiles p WHERE p.user_id = u.id) AS preferred_name";

impl CurrentUser {
    pub fn is_staff(&self) -> bool {
        matches!(self.role, Role::Staff | Role::Admin)
    }
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("hash password: {e}"))
}

fn verify_password(password: &str, hash: &str) -> bool {
    Argon2::default().verify_password(password.as_bytes(), hash).is_ok()
}

fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes[..]);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Any signed-in user.
impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar.get(SESSION_COOKIE).map(|c| c.value().to_string()).ok_or(AppError::Unauthorized)?;
        let user = sqlx::query_as::<_, CurrentUser>(sqlx::AssertSqlSafe(format!(
            "UPDATE sessions s SET last_seen_at = now()
             FROM users u
             WHERE s.token_hash = $1 AND s.expires_at > now() AND u.id = s.user_id AND u.disabled_at IS NULL
             RETURNING {USER_COLUMNS}"
        )))
        .bind(token_hash(&token))
        .fetch_optional(&state.db)
        .await?;
        user.ok_or(AppError::Unauthorized)
    }
}

/// A staff member or admin (the management site).
pub struct StaffUser(pub CurrentUser);

impl FromRequestParts<AppState> for StaffUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let user = CurrentUser::from_request_parts(parts, state).await?;
        if user.is_staff() { Ok(Self(user)) } else { Err(AppError::Forbidden) }
    }
}

/// Admins only (accounts list).
pub struct AdminUser(#[allow(dead_code)] pub CurrentUser);

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let user = CurrentUser::from_request_parts(parts, state).await?;
        if user.role == Role::Admin { Ok(Self(user)) } else { Err(AppError::Forbidden) }
    }
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(sqlx::FromRow)]
struct LoginRow {
    id: Uuid,
    password_hash: String,
}

async fn login(State(state): State<AppState>, jar: CookieJar, Json(req): Json<LoginRequest>) -> AppResult<(CookieJar, Json<CurrentUser>)> {
    let row = sqlx::query_as::<_, LoginRow>(
        "SELECT id, password_hash FROM users WHERE lower(username) = lower($1) AND disabled_at IS NULL",
    )
    .bind(req.username.trim())
    .fetch_optional(&state.db)
    .await?;
    // Same answer for an unknown user and a wrong password.
    let Some(row) = row.filter(|r| verify_password(&req.password, &r.password_hash)) else {
        return Err(AppError::BadRequest("wrong username or password".into()));
    };

    let token = new_token();
    let expires = OffsetDateTime::now_utc() + Duration::days(state.config.session_days);
    sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at) VALUES ($1, $2, $3)")
        .bind(token_hash(&token))
        .bind(row.id)
        .bind(expires)
        .execute(&state.db)
        .await?;
    sqlx::query("DELETE FROM sessions WHERE expires_at < now()").execute(&state.db).await?;

    let user = sqlx::query_as::<_, CurrentUser>(sqlx::AssertSqlSafe(format!("SELECT {USER_COLUMNS} FROM users u WHERE u.id = $1")))
        .bind(row.id)
        .fetch_one(&state.db)
        .await?;

    let cookie = Cookie::build((SESSION_COOKIE, token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(state.config.cookie_secure)
        .max_age(Duration::days(state.config.session_days))
        .build();
    Ok((jar.add(cookie), Json(user)))
}

async fn logout(State(state): State<AppState>, jar: CookieJar) -> AppResult<CookieJar> {
    if let Some(c) = jar.get(SESSION_COOKIE) {
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1").bind(token_hash(c.value())).execute(&state.db).await?;
    }
    Ok(jar.remove(Cookie::build(SESSION_COOKIE).path("/").build()))
}

async fn me(user: CurrentUser) -> Json<CurrentUser> {
    Json(user)
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/login", post(login)).route("/logout", post(logout)).route("/me", get(me))
}
