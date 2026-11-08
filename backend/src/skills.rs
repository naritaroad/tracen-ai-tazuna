//! Built-in skills (docs/engineering/upgrade-plan-1.1.md §5): one place that says which tools the agent
//! has, how the prompt routes to them, who may use them, what the MCP server exposes and which tests
//! cover them. `data/skills/<name>/SKILL.md` follows the Agent Skills format (name and description
//! only); our own fields live in `data/skills/registry.json`, because the format's `metadata` is a
//! string-to-string map. The registry is checked at start: a bad entry stops the API.

use std::collections::{BTreeSet, HashSet};
use std::path::Path;
use std::sync::{OnceLock, RwLock};

use anyhow::{Context, Result, bail};
use axum::extract::{Path as UrlPath, State};
use axum::routing::{get, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::auth::{AdminUser, StaffUser};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

use crate::tools::TOOL_NAMES;

/// Tools the MCP server can offer; a skill may only name these in `mcp_tools`.
pub const MCP_TOOL_NAMES: [&str; 5] = ["search_public_rules", "list_public_events", "search_public_services", "calculate_training_plan", "submit_public_feedback"];
/// The one MCP tool that writes; every other MCP tool must be read-only.
pub const MCP_WRITE_TOOLS: [&str; 1] = ["submit_public_feedback"];
const SURFACES: [&str; 2] = ["internal_agent", "mcp"];
const ROLES: [&str; 3] = ["trainer", "staff", "admin"];

#[derive(Debug, Clone, Deserialize)]
pub struct Entry {
    pub name: String,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub mcp_tools: Vec<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    pub surfaces: Vec<String>,
    pub visibility: String,
    pub evals: Vec<String>,
    pub enabled: bool,
}

#[derive(Deserialize)]
struct RegistryFile {
    skills: Vec<Entry>,
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub entry: Entry,
    pub description: String,
    /// The `## Routing` paragraph: one numbered rule in the agent's system prompt.
    pub routing: String,
    /// The `## External use` paragraph, copied into the SKILL.md for outside agents.
    pub external: String,
}

#[derive(Debug, Clone)]
pub struct Registry {
    pub skills: Vec<Skill>,
}

impl Registry {
    /// Enabled in registry.json and not switched off on the management site.
    pub fn enabled(&self) -> impl Iterator<Item = &Skill> {
        let off = switched_off();
        self.skills.iter().filter(move |s| s.entry.enabled && !off.contains(&s.entry.name))
    }

    /// Tools the in-app agent may call, in registry order.
    pub fn agent_tools(&self) -> Vec<&str> {
        self.enabled().filter(|s| s.entry.surfaces.iter().any(|x| x == "internal_agent")).flat_map(|s| s.entry.tools.iter().map(String::as_str)).collect()
    }

    /// Tools the MCP server offers: enabled ∩ surface `mcp` ∩ visibility `public` (§5.3).
    pub fn mcp_tools(&self) -> Vec<&str> {
        self.enabled()
            .filter(|s| s.entry.visibility == "public" && s.entry.surfaces.iter().any(|x| x == "mcp"))
            .flat_map(|s| s.entry.mcp_tools.iter().map(String::as_str))
            .collect()
    }

    /// The numbered routing rules for the prompt, starting at `first`.
    pub fn routing_rules(&self, first: usize) -> String {
        self.enabled()
            .filter(|s| !s.routing.is_empty() && s.entry.surfaces.iter().any(|x| x == "internal_agent"))
            .enumerate()
            .map(|(i, s)| format!("{}. {}", first + i, s.routing.replace('\n', "\n   ")))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

static REGISTRY: OnceLock<Registry> = OnceLock::new();
/// Skills an admin switched off (`app_settings` key `skills`). The content stays in git; the
/// management site can only switch a skill off and on again.
static SWITCHED_OFF: RwLock<Vec<String>> = RwLock::new(Vec::new());
const SETTINGS_KEY: &str = "skills";

fn switched_off() -> Vec<String> {
    SWITCHED_OFF.read().map(|v| v.clone()).unwrap_or_default()
}

#[derive(Default, Serialize, Deserialize)]
struct Switches {
    #[serde(default)]
    disabled: Vec<String>,
}

/// Read the admin switches. `table` is `app_settings`, or `mcp_settings` for the read-only MCP role.
pub async fn load_switches(db: &PgPool, table: &'static str) -> sqlx::Result<()> {
    let stored: Option<Value> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT value FROM {table} WHERE key = $1")))
        .bind(SETTINGS_KEY)
        .fetch_optional(db)
        .await?;
    let switches: Switches = stored.and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    if let Ok(mut off) = SWITCHED_OFF.write() {
        *off = switches.disabled;
    }
    Ok(())
}

/// Load and check the registry once at start.
pub fn init(dir: &Path, eval_dir: &Path) -> Result<()> {
    let registry = load(dir, eval_dir)?;
    let _ = REGISTRY.set(registry);
    Ok(())
}

/// The registry loaded by [`init`]; in tests, the files in the repository.
pub fn registry() -> &'static Registry {
    REGISTRY.get_or_init(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        load(&root.join("data/skills"), &root.join("data/eval")).expect("data/skills is valid")
    })
}

/// `name: value` lines between the `---` fences, then the Markdown body.
fn front_matter(text: &str) -> Result<(Vec<(String, String)>, String)> {
    let text = text.replace("\r\n", "\n");
    let rest = text.strip_prefix("---\n").context("SKILL.md must start with '---'")?;
    let (head, body) = rest.split_once("\n---\n").context("SKILL.md front matter has no closing '---'")?;
    let mut fields = Vec::new();
    for line in head.lines().filter(|l| !l.trim().is_empty()) {
        let (k, v) = line.split_once(':').with_context(|| format!("bad front matter line '{line}'"))?;
        let v = v.trim();
        let value = if v.starts_with('"') { serde_json::from_str::<String>(v).with_context(|| format!("bad quoted value in '{line}'"))? } else { v.to_string() };
        fields.push((k.trim().to_string(), value));
    }
    Ok((fields, body.to_string()))
}

/// The paragraph under `## {heading}`, up to the next heading.
fn section(body: &str, heading: &str) -> String {
    let marker = format!("## {heading}\n");
    let Some(start) = body.find(&marker) else { return String::new() };
    let rest = &body[start + marker.len()..];
    let end = rest.find("\n## ").unwrap_or(rest.len());
    rest[..end].trim().to_string()
}

/// Agent Skills name rule: 1–64 of a-z, 0-9 and single hyphens, not at either end.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
}

/// Test ids in data/eval/*.csv (first column).
fn eval_ids(eval_dir: &Path) -> Result<HashSet<String>> {
    let mut ids = HashSet::new();
    for entry in std::fs::read_dir(eval_dir).with_context(|| format!("read {}", eval_dir.display()))? {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "csv") {
            let mut reader = csv::Reader::from_path(&path)?;
            for row in reader.records() {
                if let Some(id) = row?.get(0) {
                    ids.insert(id.to_string());
                }
            }
        }
    }
    Ok(ids)
}

pub fn load(dir: &Path, eval_dir: &Path) -> Result<Registry> {
    let path = dir.join("registry.json");
    let file: RegistryFile = serde_json::from_str(&std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?)
        .with_context(|| format!("parse {}", path.display()))?;
    let evals = eval_ids(eval_dir)?;
    let mut names = HashSet::new();
    let mut tools_seen = HashSet::new();
    let mut skills = Vec::new();

    for entry in file.skills {
        let name = entry.name.clone();
        if !valid_name(&name) {
            bail!("skill '{name}': names are 1–64 lowercase letters, digits and single hyphens");
        }
        if !names.insert(name.clone()) {
            bail!("skill '{name}' is listed twice");
        }
        let md = dir.join(&name).join("SKILL.md");
        let (fields, body) = front_matter(&std::fs::read_to_string(&md).with_context(|| format!("read {}", md.display()))?).with_context(|| format!("skill '{name}'"))?;
        let field = |k: &str| fields.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
        if field("name").as_deref() != Some(name.as_str()) {
            bail!("skill '{name}': SKILL.md name must match the folder");
        }
        let description = field("description").unwrap_or_default();
        if description.is_empty() || description.chars().count() > 1024 {
            bail!("skill '{name}': description must be 1–1024 characters");
        }
        if let Some(extra) = fields.iter().find(|(k, _)| !["name", "description", "license", "compatibility", "metadata", "allowed-tools"].contains(&k.as_str())) {
            bail!("skill '{name}': '{}' is not an Agent Skills field; put it in registry.json", extra.0);
        }

        for t in &entry.tools {
            if !TOOL_NAMES.contains(&t.as_str()) {
                bail!("skill '{name}': unknown tool '{t}'");
            }
            if !tools_seen.insert(t.clone()) {
                bail!("skill '{name}': tool '{t}' already belongs to another skill");
            }
        }
        for t in &entry.mcp_tools {
            if !MCP_TOOL_NAMES.contains(&t.as_str()) {
                bail!("skill '{name}': unknown MCP tool '{t}'");
            }
        }
        if let Some(s) = entry.surfaces.iter().find(|s| !SURFACES.contains(&s.as_str())) {
            bail!("skill '{name}': unknown surface '{s}'");
        }
        if let Some(r) = entry.roles.iter().find(|r| !ROLES.contains(&r.as_str())) {
            bail!("skill '{name}': unknown role '{r}'");
        }
        if !crate::seed::VISIBILITIES.contains(&entry.visibility.as_str()) {
            bail!("skill '{name}': visibility must be public, trainer or staff");
        }
        let on_mcp = entry.surfaces.iter().any(|s| s == "mcp");
        let on_agent = entry.surfaces.iter().any(|s| s == "internal_agent");
        if on_mcp != !entry.mcp_tools.is_empty() {
            bail!("skill '{name}': list mcp_tools exactly when the surfaces include mcp");
        }
        if on_mcp && entry.visibility != "public" {
            bail!("skill '{name}': only public skills may be offered over MCP");
        }
        if on_agent && entry.tools.is_empty() {
            bail!("skill '{name}': an internal_agent skill needs at least one tool");
        }
        if entry.evals.is_empty() {
            bail!("skill '{name}': a skill without tests cannot be enabled");
        }
        if let Some(missing) = entry.evals.iter().find(|id| !evals.contains(*id)) {
            bail!("skill '{name}': test '{missing}' is not in data/eval");
        }

        let routing = section(&body, "Routing");
        if on_agent && routing.is_empty() {
            bail!("skill '{name}': an internal_agent skill needs a '## Routing' paragraph");
        }
        let external = section(&body, "External use");
        if on_mcp && external.is_empty() {
            bail!("skill '{name}': an mcp skill needs an '## External use' paragraph");
        }
        skills.push(Skill { entry, description, routing, external });
    }

    // Every agent tool belongs to exactly one skill, so nothing runs without routing, roles and tests.
    let unowned: BTreeSet<&str> = TOOL_NAMES.iter().copied().filter(|t| !tools_seen.contains(*t)).collect();
    if !unowned.is_empty() {
        bail!("tools without a skill: {unowned:?}");
    }
    Ok(Registry { skills })
}

// ---------------------------------------------------------------------------
// Management site: list, test results and the on/off switch
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct SkillView {
    name: String,
    description: String,
    tools: Vec<String>,
    mcp_tools: Vec<String>,
    surfaces: Vec<String>,
    visibility: String,
    in_registry: bool,
    enabled: bool,
    evals: Vec<EvalState>,
}

#[derive(Serialize)]
struct EvalState {
    id: String,
    /// The latest real-model result for this test id; none if it has not run yet.
    passed: Option<bool>,
    run: Option<String>,
}

/// Latest pass/fail per test id from data/eval/results (mock runs are not evidence and are skipped).
fn latest_results() -> std::collections::HashMap<String, (bool, String)> {
    let mut out = std::collections::HashMap::new();
    let Ok(dir) = std::fs::read_dir("data/eval/results") else { return out };
    let mut files: Vec<_> = dir.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "csv")).collect();
    files.retain(|p| !p.to_string_lossy().ends_with("-mock.csv"));
    files.sort();
    for path in files {
        let Ok(mut reader) = csv::Reader::from_path(&path) else { continue };
        let Ok(headers) = reader.headers().cloned() else { continue };
        let column = ["pass", "tool_ok"].iter().find_map(|c| headers.iter().position(|h| h == *c));
        let Some(column) = column else { continue };
        let run = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        for row in reader.records().flatten() {
            if let (Some(id), Some(v)) = (row.get(0), row.get(column)) {
                out.insert(id.to_string(), (v == "true", run.clone()));
            }
        }
    }
    out
}

async fn list(_staff: StaffUser) -> AppResult<Json<Vec<SkillView>>> {
    let results = latest_results();
    let off = switched_off();
    Ok(Json(
        registry()
            .skills
            .iter()
            .map(|s| SkillView {
                name: s.entry.name.clone(),
                description: s.description.clone(),
                tools: s.entry.tools.clone(),
                mcp_tools: s.entry.mcp_tools.clone(),
                surfaces: s.entry.surfaces.clone(),
                visibility: s.entry.visibility.clone(),
                in_registry: s.entry.enabled,
                enabled: s.entry.enabled && !off.contains(&s.entry.name),
                evals: s
                    .entry
                    .evals
                    .iter()
                    .map(|id| {
                        let r = results.get(id);
                        EvalState { id: id.clone(), passed: r.map(|r| r.0), run: r.map(|r| r.1.clone()) }
                    })
                    .collect(),
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
struct Switch {
    enabled: bool,
}

async fn switch(State(state): State<AppState>, _admin: AdminUser, UrlPath(name): UrlPath<String>, Json(s): Json<Switch>) -> AppResult<Json<Value>> {
    let Some(skill) = registry().skills.iter().find(|k| k.entry.name == name) else { return Err(AppError::NotFound("skill")) };
    if s.enabled && !skill.entry.enabled {
        return Err(AppError::BadRequest("this skill is off in data/skills/registry.json; change it there".into()));
    }
    let mut off = switched_off();
    off.retain(|n| n != &name);
    if !s.enabled {
        off.push(name.clone());
    }
    off.sort();
    sqlx::query("INSERT INTO app_settings (key, value) VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value")
        .bind(SETTINGS_KEY)
        .bind(json!({ "disabled": off }))
        .execute(&state.db)
        .await?;
    load_switches(&state.db, "app_settings").await?;
    Ok(Json(json!({ "ok": true, "enabled": s.enabled })))
}

pub fn admin_routes() -> Router<AppState> {
    Router::new().route("/skills", get(list)).route("/skills/{name}", put(switch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_repository_registry_is_valid() {
        let r = registry();
        assert_eq!(r.agent_tools().len(), TOOL_NAMES.len());
        assert!(r.mcp_tools().contains(&"search_public_rules"));
        assert!(!r.mcp_tools().contains(&"transfer_to_director"), "hand-over must never reach MCP");
        let rules = r.routing_rules(4);
        assert!(rules.starts_with("4. ") && rules.contains("\n9. "), "six routing rules, 4 to 9");
    }

    #[test]
    fn names_follow_the_agent_skills_rule() {
        assert!(valid_name("academy-rules"));
        for bad in ["Academy", "-a", "a-", "a--b", "", "a_b"] {
            assert!(!valid_name(bad), "{bad}");
        }
    }

    #[test]
    fn front_matter_reads_quoted_values() {
        let (fields, body) = front_matter("---\nname: x\ndescription: \"a: b\"\n---\n\n## Routing\n\nrule\n").unwrap();
        assert_eq!(fields[1].1, "a: b");
        assert_eq!(section(&body, "Routing"), "rule");
    }
}
