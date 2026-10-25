//! Tool-choice test: each question runs in a fresh conversation, and the
//! tools the model called are compared with the tools we accept. Results go to data/eval/results/.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;
use uuid::Uuid;

use crate::agent::{self, RunInput, StreamEvent};
use crate::auth::CurrentUser;
use crate::locale::Locale;
use crate::settings;
use crate::state::AppState;

#[derive(Deserialize)]
struct Case {
    id: String,
    kind: String,
    question: String,
    accept_tools: String,
    expect: String,
}

fn csv_field(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\"").replace('\n', " "))
}

pub async fn run(state: &AppState, path: &str, username: &str) -> Result<()> {
    let user = sqlx::query_as::<_, CurrentUser>(sqlx::AssertSqlSafe(format!("SELECT {} FROM users u WHERE lower(u.username) = lower($1)", crate::auth::USER_COLUMNS)))
        .bind(username)
        .fetch_optional(&state.db)
        .await?
        .with_context(|| format!("user {username} not found; set SEED_DEMO_ACCOUNTS=true"))?;
    let delay = Duration::from_millis(std::env::var("EVAL_DELAY_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(0));

    let mut reader = csv::Reader::from_path(path).with_context(|| format!("open {path}"))?;
    let cases: Vec<Case> = reader.deserialize().collect::<Result<_, _>>()?;
    let ai_settings = settings::load(&state.db, &state.config).await?;
    let model = ai_settings.chat_model.clone();
    let mut report = String::from("id,kind,question,accept_tools,called_tools,tool_ok,value_ok,answer\n");
    let mut passed = 0;

    for case in &cases {
        let conversation_id: Uuid = sqlx::query_scalar("INSERT INTO conversations (user_id, title) VALUES ($1, $2) RETURNING id")
            .bind(user.id)
            .bind(format!("[eval] {}", case.id))
            .fetch_one(&state.db)
            .await?;
        sqlx::query("INSERT INTO messages (conversation_id, role, content, author_user_id) VALUES ($1, 'user', $2, $3)")
            .bind(conversation_id)
            .bind(&case.question)
            .bind(user.id)
            .execute(&state.db)
            .await?;

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamEvent>();
        let input = RunInput {
            db: &state.db,
            ai: &state.ai,
            settings: &ai_settings,
            user: &user,
            conversation_id,
            locale: Locale::detect(&case.question, Locale::ZhTw),
        };
        agent::run(input, &tx).await?;
        drop(tx);

        let mut called = Vec::new();
        let mut evidence = String::new();
        let mut answer = String::new();
        while let Some(e) = rx.recv().await {
            match e.kind {
                "tool.started" => called.push(e.data["name"].as_str().unwrap_or_default().to_string()),
                "tool.completed" => evidence.push_str(&e.data["result"].to_string()),
                "message.completed" => answer = e.data["content"].as_str().unwrap_or_default().to_string(),
                "run.failed" => answer = format!("[run.failed {}]", e.data["code"]),
                _ => {}
            }
        }

        let accept: BTreeSet<&str> = case.accept_tools.split('|').filter(|t| !t.is_empty()).collect();
        let used: BTreeSet<&str> = called.iter().map(String::as_str).collect();
        let tool_ok = if accept.is_empty() { used.is_empty() } else { !used.is_empty() && used.is_subset(&accept) };
        evidence.push_str(&answer);
        let value_ok = case.expect.is_empty() || evidence.contains(&case.expect);
        if tool_ok {
            passed += 1;
        }
        println!("{:<4} {:<11} tool {:<4} value {:<4} {:?} <- {}", case.id, case.kind, if tool_ok { "ok" } else { "MISS" }, if value_ok { "ok" } else { "MISS" }, called, case.question);
        writeln!(
            report,
            "{},{},{},{},{},{},{},{}",
            case.id,
            case.kind,
            csv_field(&case.question),
            csv_field(&case.accept_tools),
            csv_field(&called.join("|")),
            tool_ok,
            value_ok,
            csv_field(&answer)
        )?;
        tokio::time::sleep(delay).await;
    }

    println!("\ntool choice: {passed}/{} with model {model}", cases.len());
    std::fs::create_dir_all("data/eval/results")?;
    let stamp = time::OffsetDateTime::now_utc().unix_timestamp();
    let name = model.replace(['/', ':'], "_");
    let out = format!("data/eval/results/{stamp}-{name}.csv");
    std::fs::write(&out, report)?;
    println!("saved {out}");
    Ok(())
}
