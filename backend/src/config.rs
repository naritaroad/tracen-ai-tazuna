//! Settings from the environment (`.env` in the project root during development).

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedMode {
    /// Rebuild the demo catalog from CSV on every start.
    Reset,
    /// Import the CSV only when the catalog is empty, so admin edits survive restarts.
    IfEmpty,
}

#[derive(Debug, Clone)]
pub struct DemoAccounts {
    pub trainer_password: String,
    pub staff_password: String,
    pub admin_password: String,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub database_url: String,
    pub seed_dir: PathBuf,
    pub seed_mode: SeedMode,
    pub demo_accounts: Option<DemoAccounts>,
    /// `mock` (offline, deterministic) or a genai model name such as `claude-...` / `gemini-...`.
    pub model: String,
    /// Starting value for the agent's steps; the management site can change it afterwards.
    pub max_steps: usize,
    pub web_dir: Option<PathBuf>,
    pub characters_dir: PathBuf,
    /// Local folder with the character runtime files; unset in public deployments.
    pub avatar_asset_dir: Option<PathBuf>,
    /// Local folder with the game-derived motion library (`game_motions.json` + `vrma/`); research use only, never public.
    pub avatar_game_motion_dir: Option<PathBuf>,
    /// Local folder with the background scene behind the character (`scene.json` + `.glb`); game-derived, never public.
    pub avatar_scene_dir: Option<PathBuf>,
    /// The official academy crest (an SVG file); the site shows it in place of its own mark when the file exists.
    pub brand_crest_file: Option<PathBuf>,
    pub cookie_secure: bool,
    pub session_days: i64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let get = |key: &str, default: &str| env::var(key).unwrap_or_else(|_| default.to_string());

        let seed_mode = match get("CATALOG_SEED_MODE", "reset").as_str() {
            "reset" => SeedMode::Reset,
            "if_empty" => SeedMode::IfEmpty,
            other => bail!("CATALOG_SEED_MODE must be reset or if_empty, not {other}"),
        };

        let demo_accounts = if get("SEED_DEMO_ACCOUNTS", "false") == "true" {
            let password = |key: &str| -> Result<String> {
                let value = env::var(key).with_context(|| format!("{key} is required when SEED_DEMO_ACCOUNTS=true"))?;
                if value.len() < 8 {
                    bail!("{key} must be at least 8 characters");
                }
                Ok(value)
            };
            Some(DemoAccounts {
                trainer_password: password("DEMO_TRAINER_PASSWORD")?,
                staff_password: password("DEMO_STAFF_PASSWORD")?,
                admin_password: password("DEMO_ADMIN_PASSWORD")?,
            })
        } else {
            None
        };

        let optional_dir = |key: &str| env::var(key).ok().filter(|v| !v.is_empty()).map(PathBuf::from);

        Ok(Self {
            bind: get("BIND_ADDR", "127.0.0.1:8080").parse().context("BIND_ADDR")?,
            database_url: env::var("DATABASE_URL").context("DATABASE_URL is required")?,
            seed_dir: PathBuf::from(get("SEED_DIR", "data/seed")),
            seed_mode,
            demo_accounts,
            model: get("MODEL", "mock"),
            max_steps: get("AGENT_MAX_STEPS", "6").parse().context("AGENT_MAX_STEPS")?,
            web_dir: optional_dir("WEB_DIR"),
            characters_dir: PathBuf::from(get("CHARACTERS_DIR", "assets/characters")),
            avatar_asset_dir: optional_dir("AVATAR_ASSET_DIR"),
            avatar_game_motion_dir: optional_dir("AVATAR_GAME_MOTION_DIR"),
            avatar_scene_dir: optional_dir("AVATAR_SCENE_DIR"),
            // Empty turns the crest off; the default is where the redrawn file is kept.
            brand_crest_file: Some(get("BRAND_CREST_FILE", "assets/brand/tracen-crest.svg")).filter(|v| !v.is_empty()).map(PathBuf::from),
            cookie_secure: get("COOKIE_SECURE", "false") == "true",
            session_days: get("SESSION_DAYS", "14").parse().context("SESSION_DAYS")?,
        })
    }
}
