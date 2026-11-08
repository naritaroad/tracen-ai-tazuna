//! `tracen-api eval-mcp`: the MCP boundary test (§8). Runs data/eval/mcp_boundary.csv against the
//! handler, plants a canary string in private tables and checks that no response contains it, and,
//! when MCP_DATABASE_URL is set, checks that the read-only role cannot read private tables.

use std::fmt::Write as _;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{Caller, TracenMcp};

const CANARY: &str = "TZN-MCP-CANARY-91X2";

#[derive(Deserialize)]
struct Case {
    id: String,
    kind: String,
    /// A tool name, or `tools/list`.
    tool: String,
    /// JSON arguments; `{run}` is replaced with a per-run stamp so feedback text stays new.
    arguments: String,
    /// `a|b`: every one must appear in the response.
    must_contain_all: String,
    /// `a|b`: none may appear. Every response is also checked for the canary.
    must_not_contain: String,
}

fn csv_field(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\"").replace('\n', " "))
}

fn split(text: &str) -> Vec<&str> {
    text.split('|').map(str::trim).filter(|t| !t.is_empty()).collect()
}

/// Put the canary into private places an outside agent must never reach.
async fn plant_canary(main: &PgPool) -> Result<()> {
    let user: Option<uuid::Uuid> = sqlx::query_scalar("SELECT id FROM users ORDER BY created_at LIMIT 1").fetch_optional(main).await?;
    if let Some(user) = user {
        sqlx::query("INSERT INTO memory_items (user_id, kind, content, source) VALUES ($1, 'preference', $2, 'trainer')")
            .bind(user)
            .bind(format!("[eval-mcp] {CANARY} private preference"))
            .execute(main)
            .await
            .context("plant canary in memory_items")?;
    }
    sqlx::query(
        "INSERT INTO handbook_chunks (doc, section, part, lang, chapter, title, content, content_hash, visibility, source_type, source_ref, sort_order)
         VALUES ('handbook', '§99.9', 0, 'zh-TW', 'eval', '[eval-mcp] 內部規定', $1, 'eval', 'trainer', 'project', 'eval', 9999)
         ON CONFLICT (doc, section, part, lang) DO UPDATE SET content = EXCLUDED.content",
    )
    .bind(format!("出賽申請 校外活動申請書 內部備註 {CANARY}"))
    .execute(main)
    .await
    .context("plant canary in handbook_chunks")?;
    Ok(())
}

async fn remove_canary(main: &PgPool) {
    let _ = sqlx::query("DELETE FROM memory_items WHERE content LIKE '[eval-mcp]%'").execute(main).await;
    let _ = sqlx::query("DELETE FROM handbook_chunks WHERE section = '§99.9' AND source_ref = 'eval'").execute(main).await;
    // Test feedback would otherwise fill the review queue.
    let _ = sqlx::query("DELETE FROM external_feedback WHERE text LIKE '[eval %'").execute(main).await;
}

/// With the read-only role, private tables must refuse and the views must answer.
async fn role_check(mcp: &PgPool) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    // external_feedback grants its receipt columns (for INSERT … RETURNING); its text must stay unreadable.
    let private = [
        ("users", "1"),
        ("sessions", "1"),
        ("memory_items", "1"),
        ("memory_events", "1"),
        ("conversations", "1"),
        ("messages", "1"),
        ("director_tickets", "1"),
        ("handbook_chunks", "1"),
        ("academy_info", "1"),
        ("external_feedback", "text"),
        ("mcp_clients", "key_hash"),
    ];
    for (table, column) in private {
        let denied = sqlx::query(sqlx::AssertSqlSafe(format!("SELECT {column} FROM {table} LIMIT 1"))).fetch_optional(mcp).await.is_err();
        out.push((format!("cannot read {table}.{column}"), denied));
    }
    for view in ["mcp_handbook", "mcp_services", "mcp_events", "mcp_info", "mcp_races"] {
        let ok = sqlx::query(sqlx::AssertSqlSafe(format!("SELECT 1 FROM {view} LIMIT 1"))).fetch_optional(mcp).await.is_ok();
        out.push((format!("can read {view}"), ok));
    }
    out
}

pub async fn run(handler: &TracenMcp, main: &PgPool, readonly: Option<&PgPool>, path: &str) -> Result<()> {
    let mut reader = csv::Reader::from_path(path).with_context(|| format!("open {path}"))?;
    let cases: Vec<Case> = reader.deserialize().collect::<Result<_, _>>()?;
    let stamp = time::OffsetDateTime::now_utc().unix_timestamp().to_string();
    plant_canary(main).await?;
    let caller = Caller::local();
    let mut report = String::from("id,kind,tool,status,contains_ok,clean_ok,pass,response\n");
    let mut passed = 0;

    for case in &cases {
        let (status, response) = if case.tool == "tools/list" {
            let names: Vec<String> = handler.tools().iter().map(|t| t.name.to_string()).collect();
            ("ok".to_string(), json!({ "tools": names }))
        } else {
            let args: Value = serde_json::from_str(&case.arguments.replace("{run}", &stamp)).with_context(|| format!("{}: arguments are not JSON", case.id))?;
            let (status, value) = handler.run_tool(&case.tool, &args, &caller).await;
            (format!("{status:?}").to_lowercase(), value)
        };
        let text = response.to_string();
        let contains_ok = split(&case.must_contain_all).iter().all(|w| text.contains(w));
        let clean_ok = !text.contains(CANARY) && split(&case.must_not_contain).iter().all(|w| !text.contains(w));
        let pass = contains_ok && clean_ok;
        passed += usize::from(pass);
        println!("{:<4} {:<16} {} {:<12} {}", case.id, case.kind, if pass { "ok  " } else { "MISS" }, status, case.tool);
        writeln!(report, "{},{},{},{},{},{},{},{}", case.id, case.kind, case.tool, status, contains_ok, clean_ok, pass, csv_field(&text.chars().take(600).collect::<String>()))?;
    }
    remove_canary(main).await;
    println!("\nmcp boundary: {passed}/{} (canary never returned: {})", cases.len(), if passed == cases.len() { "yes" } else { "see MISS rows" });

    match readonly {
        Some(pool) => {
            let checks = role_check(pool).await;
            let ok = checks.iter().filter(|c| c.1).count();
            for (what, good) in &checks {
                println!("  role {} {what}", if *good { "ok  " } else { "MISS" });
                writeln!(report, "role,{},,,,,{},", csv_field(what), good)?;
            }
            println!("read-only role: {ok}/{} checks", checks.len());
        }
        None => println!("read-only role: not checked (set MCP_DATABASE_URL to the tracen_mcp role to check it)"),
    }

    std::fs::create_dir_all("data/eval/results")?;
    let out = format!("data/eval/results/{stamp}-mcp-{}.csv", if readonly.is_some() { "readonly" } else { "main" });
    std::fs::write(&out, report)?;
    println!("saved {out}");
    Ok(())
}
