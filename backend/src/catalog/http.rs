//! HTTP endpoints for the manual (non-AI) pages. They call the same functions as the tools.

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use time::Date;
use time::macros::format_description;

use super::plan::{self, PlanError, PlanInput};
use super::{EventView, InfoView, ServiceFilter, ServiceView};
use crate::error::{AppError, AppResult};
use crate::locale::Locale;
use crate::state::AppState;

#[derive(Deserialize)]
struct LocaleQuery {
    locale: Option<String>,
}

fn locale_of(code: Option<&str>) -> Locale {
    code.and_then(Locale::parse).unwrap_or(Locale::ZhTw)
}

async fn info(State(state): State<AppState>, Query(q): Query<LocaleQuery>) -> AppResult<Json<Vec<InfoView>>> {
    let locale = locale_of(q.locale.as_deref());
    Ok(Json(super::list_info(&state.db).await?.iter().map(|r| r.view(locale)).collect()))
}

async fn info_topic(State(state): State<AppState>, Path(topic): Path<String>, Query(q): Query<LocaleQuery>) -> AppResult<Json<InfoView>> {
    let locale = locale_of(q.locale.as_deref());
    let row = super::get_info(&state.db, &topic).await?.ok_or(AppError::NotFound("topic"))?;
    Ok(Json(row.view(locale)))
}

#[derive(Deserialize)]
struct ServiceQuery {
    query: Option<String>,
    category: Option<String>,
    only_available: Option<bool>,
    locale: Option<String>,
}

async fn services(State(state): State<AppState>, Query(q): Query<ServiceQuery>) -> AppResult<Json<Vec<ServiceView>>> {
    let locale = locale_of(q.locale.as_deref());
    let filter = ServiceFilter { query: q.query, category: q.category, only_available: q.only_available };
    Ok(Json(super::search_services(&state.db, &filter).await?.iter().map(|r| r.view(locale)).collect()))
}

#[derive(Deserialize)]
struct EventQuery {
    from: String,
    to: Option<String>,
    locale: Option<String>,
}

fn date(text: &str) -> AppResult<Date> {
    Date::parse(text, format_description!("[year]-[month]-[day]")).map_err(|_| AppError::BadRequest(format!("'{text}' is not YYYY-MM-DD")))
}

async fn events(State(state): State<AppState>, Query(q): Query<EventQuery>) -> AppResult<Json<Vec<EventView>>> {
    let locale = locale_of(q.locale.as_deref());
    let from = date(&q.from)?;
    let to = match q.to.as_deref() {
        Some(t) if !t.is_empty() => date(t)?,
        _ => from,
    };
    if to < from || (to - from).whole_days() > 92 {
        return Err(AppError::BadRequest("the range must be 0 to 92 days".into()));
    }
    Ok(Json(super::list_events(&state.db, from, to).await?.iter().map(|r| r.view(locale)).collect()))
}

#[derive(Deserialize)]
struct PlanRequest {
    #[serde(flatten)]
    input: PlanInput,
    locale: Option<String>,
}

async fn calculate(State(state): State<AppState>, Json(req): Json<PlanRequest>) -> AppResult<Json<Value>> {
    let locale = locale_of(req.locale.as_deref());
    match plan::calculate_from_db(&state.db, &req.input, locale).await {
        Ok(r) => Ok(Json(json!(r))),
        Err(PlanError::Invalid(detail)) => Err(AppError::BadRequest(detail)),
        Err(PlanError::Database(e)) => Err(e.into()),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/info", get(info))
        .route("/info/{topic}", get(info_topic))
        .route("/services", get(services))
        .route("/events", get(events))
        .route("/plan", post(calculate))
}
