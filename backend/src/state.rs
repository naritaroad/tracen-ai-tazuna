use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;
use crate::llm::Ai;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub ai: Arc<Ai>,
    /// The read-only `tracen_mcp` pool (MCP_DATABASE_URL); remote MCP is off without it.
    pub mcp: Option<PgPool>,
}
