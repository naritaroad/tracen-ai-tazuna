//! Tracen Academy administrative information system — AI Hayakawa Tazuna (Rust API).
//!
//!   tracen-api                       serve the API (and the built web site when WEB_DIR is set)
//!   tracen-api eval-tools <csv>      run the tool-choice test against the configured model

mod admin;
mod agent;
mod auth;
mod avatar;
mod brand;
mod catalog;
mod config;
mod conversations;
mod error;
mod eval;
mod handoff;
mod llm;
mod locale;
mod memory;
mod picture;
mod seed;
mod settings;
mod speech;
mod state;
mod tools;

use std::sync::Arc;

use anyhow::{Context, Result};
use axum::Router;
use axum::http::{HeaderName, HeaderValue};
use axum::routing::{any, get};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::error::AppError;
use crate::state::AppState;

fn api_router() -> Router<AppState> {
    Router::new()
        .route("/health", get(|| async { axum::Json(json!({ "ok": true })) }))
        .nest("/auth", auth::routes())
        .nest("/catalog", catalog::http::routes())
        .merge(conversations::routes())
        .nest("/director", handoff::trainer_routes())
        .nest("/me", memory::routes().merge(picture::routes()))
        .nest("/admin", admin::routes().merge(settings::routes()))
        .nest("/admin/director", handoff::staff_routes())
        .nest("/speech", speech::routes())
        .nest("/avatar", avatar::routes())
        .fallback(any(|| async { AppError::NotFound("endpoint") }))
}

pub fn app(state: AppState) -> Router {
    // Asset routes answer 404 when AVATAR_ASSET_DIR is unset (the crest when its file is missing), instead of falling through to the SPA.
    let mut router = Router::new().nest("/api/v1", api_router()).merge(avatar::asset_routes()).merge(brand::routes());
    if let Some(dir) = state.config.web_dir.clone() {
        // Single-page app: unknown paths get index.html so client routes work on reload.
        router = router.fallback_service(ServeDir::new(&dir).fallback(ServeFile::new(dir.join("index.html"))));
    }
    router
        .layer(SetResponseHeaderLayer::overriding(HeaderName::from_static("x-content-type-options"), HeaderValue::from_static("nosniff")))
        .layer(SetResponseHeaderLayer::overriding(HeaderName::from_static("referrer-policy"), HeaderValue::from_static("same-origin")))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn connect(config: &Config) -> Result<sqlx::PgPool> {
    let db = PgPoolOptions::new().max_connections(10).connect(&config.database_url).await.context("connect to PostgreSQL")?;
    sqlx::migrate!("./migrations").run(&db).await.context("run migrations")?;
    let report = seed::import_catalog(&db, &config.seed_dir, config.seed_mode).await.context("import demo catalog")?;
    if report.skipped {
        tracing::info!("catalog kept (CATALOG_SEED_MODE=if_empty)");
    } else {
        tracing::info!(topics = report.topics, services = report.services, events = report.events, races = report.races, "catalog rebuilt from CSV");
    }
    seed::ensure_demo_accounts(&db, config).await?;
    Ok(db)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Relative paths (data/seed, assets/characters, web/dist) are relative to the folder of .env.
    if let Ok(path) = dotenvy::dotenv()
        && let Some(root) = path.parent()
    {
        std::env::set_current_dir(root).context("enter project root")?;
    }
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,tower_http=info,sqlx=warn")))
        .init();

    let config = Arc::new(Config::from_env()?);
    let db = connect(&config).await?;
    let ai = Arc::new(llm::Ai::new()?);
    let state = AppState { db, config: config.clone(), ai };

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("eval-tools") {
        let csv = args.get(1).map_or("data/eval/tool_choice.csv", String::as_str);
        let user = args.get(2).map_or("trainer", String::as_str);
        return eval::run(&state, csv, user).await;
    }

    tracing::info!(addr = %config.bind, default_model = %config.model, "listening");
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    axum::serve(listener, app(state)).with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    Ok(())
}
