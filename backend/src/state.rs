use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;
use crate::llm::Ai;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub ai: Arc<Ai>,
}
