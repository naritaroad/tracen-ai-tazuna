//! Manual page for the trainer handbook. It calls the same search as the agent tool.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use super::{Audience, DEFAULT_TOP_K, SearchOutcome, SearchRequest, Section};
use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use crate::locale::Locale;
use crate::settings;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/", get(list)).route("/search", get(search))
}

fn audience(user: &CurrentUser) -> Audience {
    if user.is_staff() { Audience::Staff } else { Audience::Trainer }
}

fn locale_of(code: Option<&str>) -> Locale {
    code.and_then(Locale::parse).unwrap_or(Locale::ZhTw)
}

#[derive(Deserialize)]
struct ListQuery {
    locale: Option<String>,
}

async fn list(State(state): State<AppState>, user: CurrentUser, Query(q): Query<ListQuery>) -> AppResult<Json<Vec<Section>>> {
    Ok(Json(super::sections(&state.db, locale_of(q.locale.as_deref()), audience(&user)).await?))
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    top_k: Option<usize>,
    locale: Option<String>,
}

async fn search(State(state): State<AppState>, user: CurrentUser, Query(q): Query<SearchQuery>) -> AppResult<Json<SearchOutcome>> {
    let query = q.q.trim();
    if query.is_empty() || query.chars().count() > 300 {
        return Err(AppError::BadRequest("q must be 1 to 300 characters".into()));
    }
    let settings = settings::load(&state.db, &state.config).await?;
    let request = SearchRequest {
        query,
        top_k: q.top_k.unwrap_or(DEFAULT_TOP_K),
        locale: locale_of(q.locale.as_deref()),
        audience: audience(&user),
        user_id: Some(user.id),
        purpose: "rag",
        max_distance: None,
        source: super::Source::Main,
        allow_embed: true,
    };
    Ok(Json(super::search(&state.db, &state.ai, &settings, &request).await?))
}
