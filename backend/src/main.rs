//! Tracen Academy administrative information system — AI Hayakawa Tazuna (Rust API).
//!
//!   tracen-api                       serve the API (and the built web site when WEB_DIR is set)
//!   tracen-api eval-tools <csv>      run the tool-choice test against the configured model
//!   tracen-api eval-rag <csv>        run the handbook retrieval test
//!   tracen-api eval-boundary <csv>   run the scope and no-invention test
//!   tracen-api eval-mcp <csv>        run the MCP boundary test (and the read-only role check)
//!   tracen-api mcp-stdio             serve the public MCP tools over stdin/stdout
//!   tracen-api export-skills         write skills/tracen-public/SKILL.md from data/skills

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
mod feedback;
mod handbook;
mod handoff;
mod llm;
mod locale;
mod mcp;
mod memory;
mod picture;
mod seed;
mod settings;
mod skills;
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
        .nest("/handbook", handbook::http::routes())
        .merge(conversations::routes())
        .nest("/director", handoff::trainer_routes())
        .nest("/me", memory::routes().merge(picture::routes()))
        .nest(
            "/admin",
            admin::routes().merge(settings::routes()).merge(feedback::admin_routes()).merge(mcp::admin::routes()).merge(skills::admin_routes()),
        )
        .nest("/admin/director", handoff::staff_routes())
        .nest("/speech", speech::routes())
        .nest("/avatar", avatar::routes())
        .fallback(any(|| async { AppError::NotFound("endpoint") }))
}

pub fn app(state: AppState) -> Router {
    // Asset routes answer 404 when AVATAR_ASSET_DIR is unset (the crest when its file is missing), instead of falling through to the SPA.
    let mut router = Router::new().nest("/api/v1", api_router()).merge(avatar::asset_routes()).merge(brand::routes());
    router = match state.mcp.clone() {
        // Remote MCP only with the read-only role, and only while an admin has it switched on.
        Some(readonly) => {
            let handler = mcp::TracenMcp::new(readonly, state.ai.clone(), state.config.clone(), mcp::Transport::Http);
            router.nest_service("/mcp", mcp_gate(state.clone(), mcp::http_service(handler)))
        }
        None => router.route(
            "/mcp",
            any(|| async {
                (
                    axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    axum::Json(json!({ "error": "mcp_not_configured", "detail": "Remote MCP needs MCP_DATABASE_URL (the read-only tracen_mcp role)." })),
                )
            }),
        ),
    };
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

/// The admin switch in front of `/mcp`: 503 while remote access is off.
fn mcp_gate(
    state: AppState,
    service: rmcp::transport::streamable_http_server::StreamableHttpService<mcp::TracenMcp, rmcp::transport::streamable_http_server::session::local::LocalSessionManager>,
) -> Router {
    Router::new().fallback_service(service).layer(axum::middleware::from_fn(move |req: axum::extract::Request, next: axum::middleware::Next| {
        let db = state.db.clone();
        async move {
            let on = mcp::load_settings(&db).await.map(|s| s.enabled).unwrap_or(false);
            if on {
                next.run(req).await
            } else {
                use axum::response::IntoResponse;
                (axum::http::StatusCode::SERVICE_UNAVAILABLE, axum::Json(json!({ "error": "disabled", "detail": "Remote MCP access is switched off." }))).into_response()
            }
        }
    }))
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

/// Index the handbook and academy information for search; errors are logged, never fatal.
async fn sync_handbook(state: &AppState) {
    let settings = match settings::load(&state.db, &state.config).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "handbook: could not load AI settings");
            return;
        }
    };
    match handbook::sync(&state.db, &state.ai, &settings, &state.config.rag_dir).await {
        Ok(r) => tracing::info!(chunks = r.chunks, embedded = r.embedded, without_vector = r.without_vector, "handbook indexed"),
        Err(e) => tracing::error!(error = ?e, "handbook indexing failed"),
    }
}

/// Load the project's `.env`: the first one, above the working directory and then above the
/// executable, whose folder also holds `data/skills/registry.json`. A `.env` of some other project
/// higher up (for example in the home folder) is skipped.
fn project_env() -> Option<std::path::PathBuf> {
    let cwd = std::env::current_dir().ok();
    let exe = std::env::current_exe().ok();
    let dirs = cwd.iter().flat_map(|d| d.ancestors()).chain(exe.iter().flat_map(|e| e.ancestors().skip(1)));
    let path = dirs.map(|dir| dir.join(".env")).find(|p| p.is_file() && p.with_file_name("data").join("skills").join("registry.json").is_file())?;
    dotenvy::from_path(&path).ok()?;
    Some(path)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Relative paths (data/seed, assets/characters, web/dist) are relative to the folder of .env.
    // TRACEN_HOME names that folder; otherwise the project's .env is looked up from the working
    // directory and then from the executable's folders, so an MCP client can start
    // backend/target/*/tracen-api from anywhere (plugins/tracen-academy).
    if let Some(home) = std::env::var_os("TRACEN_HOME") {
        std::env::set_current_dir(&home).context("enter TRACEN_HOME")?;
    }
    if let Some(path) = project_env().or_else(|| dotenvy::dotenv().ok())
        && let Some(root) = path.parent()
    {
        std::env::set_current_dir(root).context("enter project root")?;
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str);
    // Over stdio, stdout carries the MCP messages; logs must go to stderr.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,tower_http=info,sqlx=warn")))
        .with_writer(std::io::stderr)
        .init();

    let config = Arc::new(Config::from_env()?);
    // A broken handbook file or skill stops the start, like a broken CSV.
    handbook::load_files(&config.rag_dir).context("check the trainer handbook")?;
    skills::init(&config.skills_dir, std::path::Path::new("data/eval")).context("check data/skills")?;
    if command == Some("export-skills") {
        for path in mcp::export::write(skills::registry(), std::path::Path::new("."))? {
            println!("wrote {}", path.display());
        }
        return Ok(());
    }
    let ai = Arc::new(llm::Ai::new()?);
    let readonly = mcp::connect_readonly().await?;

    if command == Some("mcp-stdio") {
        // With the read-only role the process never touches the full tables, not even to migrate.
        let db = match readonly {
            Some(pool) => {
                skills::load_switches(&pool, "mcp_settings").await?;
                pool
            }
            None => {
                let db = connect(&config).await?;
                skills::load_switches(&db, "app_settings").await?;
                db
            }
        };
        tracing::info!(tools = ?skills::registry().mcp_tools(), "MCP over stdio");
        return mcp::serve_stdio(mcp::TracenMcp::new(db, ai, config, mcp::Transport::Stdio)).await;
    }

    let db = connect(&config).await?;
    skills::load_switches(&db, "app_settings").await?;
    match feedback::purge_rejected(&db).await {
        Ok(n) if n > 0 => tracing::info!(purged = n, "emptied rejected feedback older than 30 days"),
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "could not purge old feedback"),
    }
    let state = AppState { db, config: config.clone(), ai, mcp: readonly };
    if command.is_some_and(|c| c.starts_with("eval-")) {
        // Tests need the whole index before the first question.
        sync_handbook(&state).await;
    }
    match command {
        Some("eval-tools") => {
            let csv = args.get(1).map_or("data/eval/tool_choice.csv", String::as_str);
            let user = args.get(2).map_or("trainer", String::as_str);
            return eval::run(&state, csv, user).await;
        }
        Some("eval-rag") => {
            let csv = args.get(1).map_or("data/eval/rag_retrieval.csv", String::as_str);
            return eval::run_rag(&state, csv).await;
        }
        Some("eval-boundary") => {
            let csv = args.get(1).map_or("data/eval/boundary.csv", String::as_str);
            let user = args.get(2).map_or("trainer", String::as_str);
            return eval::run_boundary(&state, csv, user).await;
        }
        Some("eval-mcp") => {
            let csv = args.get(1).map_or("data/eval/mcp_boundary.csv", String::as_str);
            let pool = state.mcp.clone().unwrap_or_else(|| state.db.clone());
            let handler = mcp::TracenMcp::new(pool, state.ai.clone(), state.config.clone(), mcp::Transport::Stdio);
            return mcp::eval::run(&handler, &state.db, state.mcp.as_ref(), csv).await;
        }
        _ => {}
    }

    // Embedding the handbook can take a while on the first start; search uses keywords until it is done.
    let background = state.clone();
    tokio::spawn(async move { sync_handbook(&background).await });

    tracing::info!(addr = %config.bind, default_model = %config.model, "listening");
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    // Client addresses feed the MCP rate limits (memory only).
    axum::serve(listener, app(state).into_make_service_with_connect_info::<std::net::SocketAddr>()).with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    Ok(())
}
