//! Automatic memory curation after each answer (Q08 B), run in the background so the trainer
//! never waits for it. One model call proposes a short event note and preference/fact changes;
//! this code checks them, removes duplicates by meaning, and keeps history when something is replaced.

use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::agent::HISTORY_LIMIT;
use crate::llm::{Ai, vector_literal};
use crate::settings::{self, AiSettings, UsageRecord};

/// Items closer than this cosine distance are treated as the same memory.
const DUPLICATE_DISTANCE: f64 = 0.08;
/// Re-summarise when this many messages have dropped out of the history window since last time.
const SUMMARY_STEP: i64 = 10;

const CURATOR_PROMPT: &str = "\
You keep the long-term notes of Hayakawa Tazuna, a secretary at Tracen Academy, about ONE trainer.
You get the trainer's existing notes and the newest exchange. Return ONLY a JSON object:
{\"event\": string or null, \"add\": [{\"kind\": \"preference\" | \"fact\", \"content\": string}], \"supersede\": [{\"id\": string, \"content\": string}]}
- event: one sentence (max 120 characters) on what the trainer asked or arranged, useful later. null for small talk.
- add: only stable things about the trainer themself: how to address them, language or format preferences,
  their role, their Uma Musume's name, recurring plans. Max 3. Write in the trainer's language.
- supersede: when the exchange changes an existing note, give that note's id and the new content.
- Never record: health or medical details of anyone, other people's contact details, passwords, keys,
  or academy facts (rules, times, fees, availability) - those live in the academy database.
- If nothing is worth keeping, return {\"event\": null, \"add\": [], \"supersede\": []}.";

const SUMMARY_PROMPT: &str = "\
Summarise the earlier part of a conversation between a trainer and Hayakawa Tazuna (Tracen Academy secretary)
in at most 8 short bullet points, in the trainer's language. Keep requests, decisions, case numbers and dates.
Do not add anything that is not in the text. Return only the bullet points.";

#[derive(Debug, Default, Deserialize)]
pub struct Proposal {
    pub event: Option<String>,
    #[serde(default)]
    pub add: Vec<NewItem>,
    #[serde(default)]
    pub supersede: Vec<Replacement>,
}

#[derive(Debug, Deserialize)]
pub struct NewItem {
    pub kind: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct Replacement {
    pub id: String,
    pub content: String,
}

/// The JSON object in a model reply, even when wrapped in ``` fences or prose.
pub fn parse_proposal(text: &str) -> Option<Proposal> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    serde_json::from_str(text.get(start..=end)?).ok()
}

static CALL_ME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:叫我|請稱呼我|稱呼我為|と呼んで|call me)\s*[「『\x22]?([^\s」』\x22，。,.!！?？]{1,20})").expect("regex"));

/// Offline curator for `mock`: the event is the question; "叫我 X" becomes a preference.
fn mock_proposal(question: &str) -> Proposal {
    let event = Some(format!("訓練員問：{}", question.chars().take(100).collect::<String>()));
    let add = CALL_ME
        .captures(question)
        .map(|c| vec![NewItem { kind: "preference".into(), content: format!("希望被稱呼為「{}」", &c[1]) }])
        .unwrap_or_default();
    Proposal { event, add, supersede: Vec::new() }
}

#[derive(sqlx::FromRow)]
struct Turn {
    role: String,
    content: String,
}

#[derive(sqlx::FromRow)]
struct Note {
    id: Uuid,
    kind: String,
    content: String,
}

/// Entry point, spawned after a successful run. Errors are logged, never shown to the trainer.
pub async fn after_run(db: PgPool, ai: Ai, settings: AiSettings, user_id: Uuid, conversation_id: Uuid, run_id: Uuid) {
    if !settings.memory_enabled {
        return;
    }
    if let Err(e) = curate(&db, &ai, &settings, user_id, conversation_id, run_id).await {
        tracing::warn!(error = ?e, %run_id, "memory curation failed");
    }
    if let Err(e) = summarise(&db, &ai, &settings, user_id, conversation_id).await {
        tracing::warn!(error = ?e, %conversation_id, "conversation summary failed");
    }
}

async fn embed_and_record(db: &PgPool, ai: &Ai, settings: &AiSettings, user_id: Uuid, text: &str) -> anyhow::Result<String> {
    let e = ai.embed(&settings.embed_model, text).await?;
    settings::record_usage(
        db,
        settings,
        UsageRecord { purpose: "embed", model: &settings.embed_model, prompt_tokens: e.prompt_tokens, completion_tokens: 0, user_id: Some(user_id), run_id: None },
    )
    .await;
    Ok(vector_literal(&e.vector))
}

async fn curate(db: &PgPool, ai: &Ai, settings: &AiSettings, user_id: Uuid, conversation_id: Uuid, run_id: Uuid) -> anyhow::Result<()> {
    if settings::check_budget(db, settings).await.is_err() {
        return Ok(());
    }
    // The answer of this run and the trainer message just before it.
    let mut turns = sqlx::query_as::<_, Turn>(
        "SELECT role, content FROM messages
         WHERE conversation_id = $1 AND role IN ('user', 'assistant')
           AND created_at <= (SELECT created_at FROM messages WHERE run_id = $2 AND role = 'assistant' LIMIT 1)
         ORDER BY created_at DESC LIMIT 2",
    )
    .bind(conversation_id)
    .bind(run_id)
    .fetch_all(db)
    .await?;
    turns.reverse();
    let question = turns.iter().find(|t| t.role == "user").map(|t| t.content.clone()).unwrap_or_default();
    if question.is_empty() {
        return Ok(());
    }
    let notes = sqlx::query_as::<_, Note>("SELECT id, kind, content FROM memory_items WHERE user_id = $1 AND status = 'active' ORDER BY created_at")
        .bind(user_id)
        .fetch_all(db)
        .await?;

    let proposal = if settings.memory_model == "mock" {
        mock_proposal(&question)
    } else {
        let existing: Vec<Value> = notes.iter().map(|n| serde_json::json!({ "id": n.id, "kind": n.kind, "content": n.content })).collect();
        let exchange: Vec<Value> = turns.iter().map(|t| serde_json::json!({ "role": t.role, "content": t.content })).collect();
        let input = serde_json::json!({ "existing_notes": existing, "newest_exchange": exchange }).to_string();
        let (text, usage) = ai.complete(&settings.memory_model, CURATOR_PROMPT, &input).await?;
        let (p, c) = settings::tokens(&usage);
        settings::record_usage(db, settings, UsageRecord { purpose: "memory", model: &settings.memory_model, prompt_tokens: p, completion_tokens: c, user_id: Some(user_id), run_id: Some(run_id) }).await;
        parse_proposal(&text).unwrap_or_default()
    };

    if let Some(event) = proposal.event.map(|e| e.trim().chars().take(500).collect::<String>()).filter(|e| !e.is_empty()) {
        let vector = embed_and_record(db, ai, settings, user_id, &event).await?;
        sqlx::query(
            "INSERT INTO memory_events (user_id, conversation_id, run_id, content, embedding, embed_model) VALUES ($1, $2, $3, $4, $5::vector, $6)",
        )
        .bind(user_id)
        .bind(conversation_id)
        .bind(run_id)
        .bind(&event)
        .bind(vector)
        .bind(&settings.embed_model)
        .execute(db)
        .await?;
    }

    for item in proposal.add.into_iter().take(3) {
        let content: String = item.content.trim().chars().take(500).collect();
        if content.is_empty() || !["preference", "fact"].contains(&item.kind.as_str()) {
            continue;
        }
        let vector = embed_and_record(db, ai, settings, user_id, &content).await?;
        let duplicate: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM memory_items WHERE user_id = $1 AND status = 'active'
                AND (content = $2 OR (embed_model = $3 AND (embedding <=> $4::vector) < $5)))",
        )
        .bind(user_id)
        .bind(&content)
        .bind(&settings.embed_model)
        .bind(&vector)
        .bind(DUPLICATE_DISTANCE)
        .fetch_one(db)
        .await?;
        if !duplicate {
            sqlx::query(
                "INSERT INTO memory_items (user_id, kind, content, source, embedding, embed_model) VALUES ($1, $2, $3, 'agent', $4::vector, $5)",
            )
            .bind(user_id)
            .bind(&item.kind)
            .bind(&content)
            .bind(&vector)
            .bind(&settings.embed_model)
            .execute(db)
            .await?;
        }
    }

    for r in proposal.supersede.into_iter().take(3) {
        // Only the trainer's own active notes can be replaced.
        let Some(old) = Uuid::parse_str(&r.id).ok().and_then(|id| notes.iter().find(|n| n.id == id)) else { continue };
        let content: String = r.content.trim().chars().take(500).collect();
        if content.is_empty() || content == old.content {
            continue;
        }
        let vector = embed_and_record(db, ai, settings, user_id, &content).await?;
        let mut tx = db.begin().await?;
        let new_id: Uuid = sqlx::query_scalar(
            "INSERT INTO memory_items (user_id, kind, content, source, embedding, embed_model) VALUES ($1, $2, $3, 'agent', $4::vector, $5) RETURNING id",
        )
        .bind(user_id)
        .bind(&old.kind)
        .bind(&content)
        .bind(&vector)
        .bind(&settings.embed_model)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("UPDATE memory_items SET status = 'superseded', superseded_by = $3, updated_at = now() WHERE id = $1 AND user_id = $2 AND status = 'active'")
            .bind(old.id)
            .bind(user_id)
            .bind(new_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
    }
    Ok(())
}

/// Keep `conversations.summary` covering the messages that no longer fit in the history window.
async fn summarise(db: &PgPool, ai: &Ai, settings: &AiSettings, user_id: Uuid, conversation_id: Uuid) -> anyhow::Result<()> {
    let (total, covered): (i64, i32) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM messages WHERE conversation_id = $1 AND role IN ('user', 'assistant', 'director')), summary_message_count
         FROM conversations WHERE id = $1",
    )
    .bind(conversation_id)
    .fetch_one(db)
    .await?;
    let outside = total - HISTORY_LIMIT;
    if outside <= 0 || outside - i64::from(covered) < SUMMARY_STEP || settings::check_budget(db, settings).await.is_err() {
        return Ok(());
    }
    let old = sqlx::query_as::<_, Turn>(
        "SELECT role, content FROM messages WHERE conversation_id = $1 AND role IN ('user', 'assistant', 'director')
         ORDER BY created_at LIMIT $2",
    )
    .bind(conversation_id)
    .bind(outside)
    .fetch_all(db)
    .await?;
    let transcript: String = old.iter().map(|t| format!("{}: {}\n", t.role, t.content)).collect();
    let summary = if settings.memory_model == "mock" {
        old.iter().filter(|t| t.role == "user").map(|t| format!("- {}", t.content.chars().take(60).collect::<String>())).collect::<Vec<_>>().join("\n")
    } else {
        let (text, usage) = ai.complete(&settings.memory_model, SUMMARY_PROMPT, &transcript).await?;
        let (p, c) = settings::tokens(&usage);
        settings::record_usage(db, settings, UsageRecord { purpose: "summary", model: &settings.memory_model, prompt_tokens: p, completion_tokens: c, user_id: Some(user_id), run_id: None }).await;
        text.trim().to_string()
    };
    if !summary.is_empty() {
        sqlx::query("UPDATE conversations SET summary = $2, summary_message_count = $3 WHERE id = $1")
            .bind(conversation_id)
            .bind(summary)
            .bind(outside as i32)
            .execute(db)
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_json_inside_fences() {
        let p = parse_proposal("```json\n{\"event\": \"asked about the pool\", \"add\": [{\"kind\": \"preference\", \"content\": \"short answers\"}]}\n```")
            .expect("proposal");
        assert_eq!(p.event.as_deref(), Some("asked about the pool"));
        assert_eq!(p.add.len(), 1);
        assert!(p.supersede.is_empty());
        assert!(parse_proposal("no json here").is_none());
    }

    #[test]
    fn mock_curator_picks_up_a_preferred_name() {
        let p = mock_proposal("以後請叫我小林訓練員，謝謝");
        assert_eq!(p.add[0].content, "希望被稱呼為「小林訓練員」");
        assert!(mock_proposal("坂路可以用嗎").add.is_empty());
    }
}
