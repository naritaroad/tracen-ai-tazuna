//! Offline-runnable tests against the configured model. Each question runs in a fresh conversation.
//! Results go to data/eval/results/ (runs with the mock model are git-ignored; they are not evidence).
//!
//! - `eval-tools`: which tools the model called, compared with the tools we accept.
//! - `eval-rag`: whether handbook search returns the expected sections (no model call, only embeddings).
//! - `eval-boundary`: scope, prompt-leak and no-invention checks on the final reply.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;
use uuid::Uuid;

use crate::agent::{self, RunInput, StreamEvent, prompt};
use crate::auth::CurrentUser;
use crate::handbook::{self, Audience, SearchRequest};
use crate::locale::Locale;
use crate::settings::{self, AiSettings};
use crate::state::AppState;

fn csv_field(text: &str) -> String {
    format!("\"{}\"", text.replace('"', "\"\"").replace('\n', " "))
}

fn delay() -> Duration {
    Duration::from_millis(std::env::var("EVAL_DELAY_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(0))
}

async fn find_user(state: &AppState, username: &str) -> Result<CurrentUser> {
    sqlx::query_as::<_, CurrentUser>(sqlx::AssertSqlSafe(format!("SELECT {} FROM users u WHERE lower(u.username) = lower($1)", crate::auth::USER_COLUMNS)))
        .bind(username)
        .fetch_optional(&state.db)
        .await?
        .with_context(|| format!("user {username} not found; set SEED_DEMO_ACCOUNTS=true"))
}

fn save(kind: &str, model: &str, report: String) -> Result<()> {
    std::fs::create_dir_all("data/eval/results")?;
    let stamp = time::OffsetDateTime::now_utc().unix_timestamp();
    let name = model.replace(['/', ':'], "_");
    let prefix = if kind.is_empty() { String::new() } else { format!("{kind}-") };
    let out = format!("data/eval/results/{stamp}-{prefix}{name}.csv");
    std::fs::write(&out, report)?;
    println!("saved {out}");
    Ok(())
}

/// What one question produced.
struct Asked {
    called: Vec<String>,
    evidence: String,
    answer: String,
}

/// Ask one question in a new conversation, as `user`.
async fn ask(state: &AppState, user: &CurrentUser, settings: &AiSettings, id: &str, question: &str) -> Result<Asked> {
    let conversation_id: Uuid = sqlx::query_scalar("INSERT INTO conversations (user_id, title) VALUES ($1, $2) RETURNING id")
        .bind(user.id)
        .bind(format!("[eval] {id}"))
        .fetch_one(&state.db)
        .await?;
    sqlx::query("INSERT INTO messages (conversation_id, role, content, author_user_id) VALUES ($1, 'user', $2, $3)")
        .bind(conversation_id)
        .bind(question)
        .bind(user.id)
        .execute(&state.db)
        .await?;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<StreamEvent>();
    let input = RunInput { db: &state.db, ai: &state.ai, settings, user, conversation_id, locale: Locale::detect(question, Locale::ZhTw) };
    agent::run(input, &tx).await?;
    drop(tx);

    let mut asked = Asked { called: Vec::new(), evidence: String::new(), answer: String::new() };
    while let Some(e) = rx.recv().await {
        match e.kind {
            "tool.started" => asked.called.push(e.data["name"].as_str().unwrap_or_default().to_string()),
            "tool.completed" => asked.evidence.push_str(&e.data["result"].to_string()),
            "message.completed" => asked.answer = e.data["content"].as_str().unwrap_or_default().to_string(),
            "run.failed" => asked.answer = format!("[run.failed {}]", e.data["code"]),
            _ => {}
        }
    }
    Ok(asked)
}

/// `a|b` → [a, b]; empty means none.
fn alternatives(text: &str) -> Vec<&str> {
    text.split('|').map(str::trim).filter(|t| !t.is_empty()).collect()
}

// ---------------------------------------------------------------------------
// eval-tools
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ToolCase {
    id: String,
    kind: String,
    question: String,
    accept_tools: String,
    expect: String,
}

pub async fn run(state: &AppState, path: &str, username: &str) -> Result<()> {
    let user = find_user(state, username).await?;
    let mut reader = csv::Reader::from_path(path).with_context(|| format!("open {path}"))?;
    let cases: Vec<ToolCase> = reader.deserialize().collect::<Result<_, _>>()?;
    let ai_settings = settings::load(&state.db, &state.config).await?;
    let model = ai_settings.chat_model.clone();
    let mut report = String::from("id,kind,question,accept_tools,called_tools,tool_ok,value_ok,answer\n");
    let mut passed = 0;

    for case in &cases {
        let asked = ask(state, &user, &ai_settings, &case.id, &case.question).await?;
        let accept: BTreeSet<&str> = alternatives(&case.accept_tools).into_iter().collect();
        let used: BTreeSet<&str> = asked.called.iter().map(String::as_str).collect();
        let tool_ok = if accept.is_empty() { used.is_empty() } else { !used.is_empty() && used.is_subset(&accept) };
        let evidence = format!("{}{}", asked.evidence, asked.answer);
        let value_ok = case.expect.is_empty() || evidence.contains(&case.expect);
        if tool_ok {
            passed += 1;
        }
        println!(
            "{:<4} {:<11} tool {:<4} value {:<4} {:?} <- {}",
            case.id,
            case.kind,
            if tool_ok { "ok" } else { "MISS" },
            if value_ok { "ok" } else { "MISS" },
            asked.called,
            case.question
        );
        writeln!(
            report,
            "{},{},{},{},{},{},{},{}",
            case.id,
            case.kind,
            csv_field(&case.question),
            csv_field(&case.accept_tools),
            csv_field(&asked.called.join("|")),
            tool_ok,
            value_ok,
            csv_field(&asked.answer)
        )?;
        tokio::time::sleep(delay()).await;
    }

    println!("\ntool choice: {passed}/{} with model {model}", cases.len());
    save("", &model, report)
}

// ---------------------------------------------------------------------------
// eval-rag
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RagCase {
    id: String,
    lang: String,
    question: String,
    /// `§7.3|§7.2` (any of them counts) or a topic code; empty for `not_in_handbook`.
    expect_sections: String,
    /// `in_handbook` or `not_in_handbook`.
    kind: String,
}

pub async fn run_rag(state: &AppState, path: &str) -> Result<()> {
    let mut reader = csv::Reader::from_path(path).with_context(|| format!("open {path}"))?;
    let cases: Vec<RagCase> = reader.deserialize().collect::<Result<_, _>>()?;
    let ai_settings = settings::load(&state.db, &state.config).await?;
    let model = ai_settings.embed_model.clone();
    let cut = handbook::max_distance(&model);
    let mut report = String::from("id,kind,lang,question,expect_sections,top_sections,top_distances,hit_at_3,found,pass\n");
    let (mut hits, mut in_total, mut rejects, mut out_total) = (0, 0, 0, 0);
    let mut in_best: Vec<f64> = Vec::new();
    let mut out_best: Vec<f64> = Vec::new();

    for case in &cases {
        let locale = Locale::parse(&case.lang).unwrap_or(Locale::ZhTw);
        // Look past the cut-off so the report shows where the threshold should sit.
        let request = SearchRequest {
            query: &case.question,
            top_k: 3,
            locale,
            audience: Audience::Trainer,
            user_id: None,
            purpose: "rag",
            max_distance: Some(2.0),
            source: handbook::Source::Main,
            allow_embed: true,
        };
        let outcome = handbook::search(&state.db, &state.ai, &ai_settings, &request).await?;
        let cutoff = if outcome.method == "vector" { cut } else { 1.0 - handbook::KEYWORD_MIN_SCORE };
        let sections: Vec<String> = outcome.results.iter().map(|h| h.section.clone()).collect();
        let distances: Vec<f64> = outcome.results.iter().map(|h| h.distance).collect();
        let best = distances.first().copied().unwrap_or(2.0);
        let found = best <= cutoff;
        let expected = alternatives(&case.expect_sections);
        let hit = outcome.results.iter().filter(|h| h.distance <= cutoff).any(|h| expected.contains(&h.section.as_str()));
        let pass = if case.kind == "in_handbook" {
            in_total += 1;
            in_best.push(best);
            hits += usize::from(hit);
            hit
        } else {
            out_total += 1;
            out_best.push(best);
            rejects += usize::from(!found);
            !found
        };
        println!("{:<4} {:<15} {} {} {:?} {:?} <- {}", case.id, case.kind, if pass { "ok  " } else { "MISS" }, outcome.method, sections, distances, case.question);
        writeln!(
            report,
            "{},{},{},{},{},{},{},{},{},{}",
            case.id,
            case.kind,
            case.lang,
            csv_field(&case.question),
            csv_field(&case.expect_sections),
            csv_field(&sections.join("|")),
            csv_field(&distances.iter().map(|d| format!("{d:.3}")).collect::<Vec<_>>().join("|")),
            hit,
            found,
            pass
        )?;
        tokio::time::sleep(delay()).await;
    }

    let spread = |v: &[f64]| {
        let mut v = v.to_vec();
        v.sort_by(f64::total_cmp);
        match (v.first(), v.last()) {
            (Some(lo), Some(hi)) => format!("best distances {lo:.3}–{hi:.3}"),
            _ => "none".into(),
        }
    };
    println!("\nhandbook retrieval with {model} (cut-off {cut:.2}):");
    println!("  in handbook: {hits}/{in_total} have an expected section in the top 3 ({})", spread(&in_best));
    println!("  not in handbook: {rejects}/{out_total} correctly found nothing ({})", spread(&out_best));
    save("rag", &model, report)
}

// ---------------------------------------------------------------------------
// eval-boundary
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct BoundaryCase {
    id: String,
    kind: String,
    question: String,
    /// `-` = no tool may run; empty = any; otherwise the allowed tools.
    accept_tools: String,
    /// One of these must appear in the reply. `NOT_FOUND`, `OUT_OF_SCOPE` and `KEEP_RULES` stand for
    /// the fixed sentences in the reply's language.
    must_contain: String,
    /// None of these may appear in the reply. `CANARY` stands for the internal marker.
    must_not_contain: String,
}

fn expand(token: &str, locale: Locale) -> String {
    match token {
        "NOT_FOUND" => prompt::fixed_sentence(locale, 0).to_string(),
        "OUT_OF_SCOPE" => prompt::fixed_sentence(locale, 1).to_string(),
        "KEEP_RULES" => prompt::fixed_sentence(locale, 2).to_string(),
        "CANARY" => prompt::CANARY.to_string(),
        other => other.to_string(),
    }
}

/// Compared without spaces and punctuation, so a changed comma does not fail a test.
fn normalise(text: &str) -> String {
    text.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

pub async fn run_boundary(state: &AppState, path: &str, username: &str) -> Result<()> {
    let user = find_user(state, username).await?;
    let mut reader = csv::Reader::from_path(path).with_context(|| format!("open {path}"))?;
    let cases: Vec<BoundaryCase> = reader.deserialize().collect::<Result<_, _>>()?;
    let ai_settings = settings::load(&state.db, &state.config).await?;
    let model = ai_settings.chat_model.clone();
    let mut report = String::from("id,kind,question,called_tools,tool_ok,contains_ok,clean_ok,pass,answer\n");
    let mut passed = 0;

    for case in &cases {
        let locale = Locale::detect(&case.question, Locale::ZhTw);
        let asked = ask(state, &user, &ai_settings, &case.id, &case.question).await?;
        let used: BTreeSet<&str> = asked.called.iter().map(String::as_str).collect();
        let tool_ok = match case.accept_tools.trim() {
            "" => true,
            "-" => used.is_empty(),
            list => used.iter().all(|t| alternatives(list).contains(t)),
        };
        let reply = normalise(&asked.answer);
        let wanted = alternatives(&case.must_contain);
        let contains_ok = wanted.is_empty() || wanted.iter().any(|w| reply.contains(&normalise(&expand(w, locale))));
        let clean_ok = alternatives(&case.must_not_contain).iter().all(|w| !reply.contains(&normalise(&expand(w, locale))));
        let pass = tool_ok && contains_ok && clean_ok;
        passed += usize::from(pass);
        println!(
            "{:<4} {:<13} {} tool {:<4} says {:<4} clean {:<4} {:?} <- {}",
            case.id,
            case.kind,
            if pass { "ok  " } else { "MISS" },
            if tool_ok { "ok" } else { "MISS" },
            if contains_ok { "ok" } else { "MISS" },
            if clean_ok { "ok" } else { "MISS" },
            asked.called,
            case.question
        );
        writeln!(
            report,
            "{},{},{},{},{},{},{},{},{}",
            case.id,
            case.kind,
            csv_field(&case.question),
            csv_field(&asked.called.join("|")),
            tool_ok,
            contains_ok,
            clean_ok,
            pass,
            csv_field(&asked.answer)
        )?;
        tokio::time::sleep(delay()).await;
    }

    println!("\nboundary: {passed}/{} with model {model}", cases.len());
    save("boundary", &model, report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_sentences_match_despite_punctuation() {
        let said = "學園的資料和訓練員手冊裡都沒有這一項 我不能自己判斷 需要的話 我可以幫您轉給理事長室！";
        assert!(normalise(said).contains(&normalise(&expand("NOT_FOUND", Locale::ZhTw))));
        assert_eq!(expand("CANARY", Locale::EnGb), prompt::CANARY);
    }
}
