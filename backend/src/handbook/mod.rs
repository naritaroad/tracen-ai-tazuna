//! Trainer handbook RAG (docs/engineering/upgrade-plan-1.1.md §4): parse the handbook files, keep
//! `handbook_chunks` in step with them and with `academy_info`, and search the chunks.
//! The agent tool, the manual page and the MCP server all call [`search`].

pub mod http;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::catalog::{self, InfoRow};
use crate::llm::{Ai, vector_literal};
use crate::locale::Locale;
use crate::settings::{self, AiSettings, UsageRecord};

/// Sections longer than this are split at paragraph breaks.
const MAX_CHUNK_CHARS: usize = 800;
pub const DEFAULT_TOP_K: usize = 3;
pub const MAX_TOP_K: usize = 5;
/// Nearest neighbours fetched before sections are de-duplicated.
const CANDIDATES: i64 = 40;

/// One file per language, with the same section numbers in each.
pub const FILES: [(&str, &str); 3] = [("zh-TW", "trainer_handbook.zh-TW.md"), ("ja", "trainer_handbook.ja.md"), ("en-GB", "trainer_handbook.en.md")];

/// Hybrid distance above which a chunk does not count as an answer. Tuned with `eval-rag`
/// (data/eval/rag_retrieval.csv, 2026-11-07, gemini-embedding-2): every in-handbook question had its
/// best section at 0.31 or closer, and half of the off-topic ones were farther. The other half sit
/// inside the range, so the model still judges whether a returned section answers the question.
/// The offline mock embedding is far coarser than a real one.
pub fn max_distance(embed_model: &str) -> f64 {
    if embed_model == "mock" { 0.80 } else { 0.31 }
}

/// How much the keyword score (0–1) lowers a vector distance in the hybrid ranking.
const KEYWORD_WEIGHT: f64 = 0.15;

/// Share of the query's character pairs that a chunk must contain in keyword mode.
pub const KEYWORD_MIN_SCORE: f64 = 0.45;

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub doc: &'static str,
    pub section: String,
    pub part: i16,
    pub lang: &'static str,
    pub chapter: String,
    pub title: String,
    pub content: String,
    pub visibility: String,
    pub source_type: String,
    pub source_ref: String,
    pub sort_order: i32,
}

/// Document and query formats recommended for `gemini-embedding-2`, which takes the task as text
/// instead of a `task_type` field (https://ai.google.dev/gemini-api/docs/embeddings, 2026-11-07).
fn document_text(chapter: &str, title: &str, content: &str) -> String {
    format!("title: {} {} | text: {}", chapter.trim(), title.trim(), content.trim())
}

fn query_text(query: &str) -> String {
    format!("task: question answering | query: {}", query.trim())
}

impl Chunk {
    fn embed_text(&self) -> String {
        document_text(&self.chapter, &self.title, &self.content)
    }

    fn hash(&self) -> String {
        hex(&Sha256::digest(self.embed_text().as_bytes()))
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Remove HTML comments, keeping `visibility: …` comments as marker lines.
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 4..];
        let Some(end) = after.find("-->") else {
            rest = "";
            break;
        };
        let inner = after[..end].trim();
        if let Some(value) = inner.strip_prefix("visibility:") {
            out.push_str(&format!("\u{1}visibility={}", value.trim()));
        }
        rest = &after[end + 3..];
    }
    out.push_str(rest);
    out
}

/// `### §3.2 標題` → (`§3.2`, `標題`).
fn section_heading(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("### ")?.trim();
    let (id, title) = rest.split_once(char::is_whitespace)?;
    let number = id.strip_prefix('§')?;
    let mut parts = number.split('.');
    let ok = parts.next().is_some_and(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        && parts.next().is_some_and(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        && parts.next().is_none();
    ok.then(|| (id.to_string(), title.trim().to_string()))
}

/// Split a section body at blank lines into chunks of at most about `MAX_CHUNK_CHARS` characters.
fn split_body(body: &str) -> Vec<String> {
    let paragraphs: Vec<&str> = body.split("\n\n").map(str::trim).filter(|p| !p.is_empty()).collect();
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    for p in paragraphs {
        if !current.is_empty() && current.chars().count() + p.chars().count() > MAX_CHUNK_CHARS {
            chunks.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(p);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// Parse one handbook file. Errors name the problem so a bad edit stops the start-up import.
pub fn parse_handbook(text: &str, lang: &'static str) -> Result<Vec<Chunk>, String> {
    let text = strip_comments(&text.replace("\r\n", "\n"));
    struct Open {
        chapter: String,
        id: String,
        title: String,
        visibility: String,
        body: String,
    }
    let mut sections: Vec<Open> = Vec::new();
    let mut chapter = String::new();
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("## ") {
            chapter = name.trim().to_string();
        } else if line.starts_with("### ") {
            let (id, title) = section_heading(line).ok_or_else(|| format!("{lang}: bad section heading '{line}' (expected '### §N.M title')"))?;
            sections.push(Open { chapter: chapter.clone(), id, title, visibility: "trainer".into(), body: String::new() });
        } else if let Some(value) = line.trim().strip_prefix("\u{1}visibility=") {
            let Some(open) = sections.last_mut() else { continue };
            if !crate::seed::VISIBILITIES.contains(&value) {
                return Err(format!("{lang} {}: unknown visibility '{value}'", open.id));
            }
            open.visibility = value.to_string();
        } else if let Some(open) = sections.last_mut() {
            open.body.push_str(line);
            open.body.push('\n');
        }
    }

    let mut seen = HashSet::new();
    let mut chunks = Vec::new();
    for (order, s) in sections.into_iter().enumerate() {
        if !seen.insert(s.id.clone()) {
            return Err(format!("{lang}: section {} appears twice", s.id));
        }
        let parts = split_body(&s.body);
        if parts.is_empty() {
            return Err(format!("{lang} {}: the section has no text", s.id));
        }
        for (i, content) in parts.into_iter().enumerate() {
            chunks.push(Chunk {
                doc: "handbook",
                section: s.id.clone(),
                part: i as i16,
                lang,
                chapter: s.chapter.clone(),
                title: s.title.clone(),
                content,
                visibility: s.visibility.clone(),
                source_type: "project".into(),
                source_ref: format!("訓練員手冊 {}", s.id),
                sort_order: order as i32,
            });
        }
    }
    Ok(chunks)
}

/// The short information topics, indexed so one search covers them too.
fn info_chunks(rows: &[InfoRow]) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    for (order, r) in rows.iter().enumerate() {
        for (lang, chapter, title, content) in [
            ("zh-TW", "學園資訊", &r.title, &r.content),
            ("ja", "学園情報", &r.title_ja, &r.content_ja),
            ("en-GB", "Academy information", &r.title_en, &r.content_en),
        ] {
            if content.trim().is_empty() {
                continue;
            }
            chunks.push(Chunk {
                doc: "academy_info",
                section: r.topic.clone(),
                part: 0,
                lang,
                chapter: chapter.into(),
                title: title.clone(),
                content: content.clone(),
                visibility: if r.visibility.is_empty() { "trainer".into() } else { r.visibility.clone() },
                source_type: r.source_type.clone(),
                source_ref: r.source_ref.clone(),
                sort_order: 1000 + order as i32,
            });
        }
    }
    chunks
}

/// Every language file must carry the same sections as the Traditional Chinese one.
fn check_parity(by_lang: &BTreeMap<&str, Vec<Chunk>>) -> Result<(), String> {
    let ids = |lang: &str| -> Option<Vec<String>> {
        by_lang.get(lang).map(|c| {
            let mut v: Vec<String> = c.iter().filter(|c| c.part == 0).map(|c| c.section.clone()).collect();
            v.dedup();
            v
        })
    };
    let Some(reference) = ids("zh-TW") else { return Ok(()) };
    for (lang, _) in FILES {
        if let Some(other) = ids(lang)
            && other != reference
        {
            return Err(format!("{lang}: sections differ from zh-TW"));
        }
    }
    Ok(())
}

/// Read and check all handbook files in `dir`. Missing files are skipped (Q15: Chinese first).
pub fn load_files(dir: &Path) -> Result<Vec<Chunk>> {
    let mut by_lang = BTreeMap::new();
    for (lang, file) in FILES {
        let path = dir.join(file);
        if !path.exists() {
            continue;
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let chunks = parse_handbook(&text, lang).map_err(anyhow::Error::msg)?;
        by_lang.insert(lang, chunks);
    }
    check_parity(&by_lang).map_err(anyhow::Error::msg)?;
    Ok(by_lang.into_values().flatten().collect())
}

// ---------------------------------------------------------------------------
// Index
// ---------------------------------------------------------------------------

pub struct SyncReport {
    pub chunks: usize,
    pub embedded: usize,
    pub without_vector: usize,
}

/// Bring `handbook_chunks` in line with the files and `academy_info`, then embed what changed.
/// Embedding failures are logged, not fatal: search falls back to keywords.
pub async fn sync(db: &PgPool, ai: &Ai, settings: &AiSettings, dir: &Path) -> Result<SyncReport> {
    let mut chunks = load_files(dir)?;
    if chunks.is_empty() {
        tracing::warn!(dir = %dir.display(), "no trainer handbook found; only academy information is searchable");
    }
    chunks.extend(info_chunks(&catalog::list_info(db).await?));

    let mut tx = db.begin().await?;
    for c in &chunks {
        sqlx::query(
            "INSERT INTO handbook_chunks (doc, section, part, lang, chapter, title, content, content_hash, visibility, source_type, source_ref, sort_order)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
             ON CONFLICT (doc, section, part, lang) DO UPDATE SET
               chapter = EXCLUDED.chapter, title = EXCLUDED.title, content = EXCLUDED.content,
               visibility = EXCLUDED.visibility, source_type = EXCLUDED.source_type, source_ref = EXCLUDED.source_ref,
               sort_order = EXCLUDED.sort_order,
               embedding = CASE WHEN handbook_chunks.content_hash = EXCLUDED.content_hash THEN handbook_chunks.embedding END,
               embed_model = CASE WHEN handbook_chunks.content_hash = EXCLUDED.content_hash THEN handbook_chunks.embed_model END,
               content_hash = EXCLUDED.content_hash",
        )
        .bind(c.doc)
        .bind(&c.section)
        .bind(c.part)
        .bind(c.lang)
        .bind(&c.chapter)
        .bind(&c.title)
        .bind(&c.content)
        .bind(c.hash())
        .bind(&c.visibility)
        .bind(&c.source_type)
        .bind(&c.source_ref)
        .bind(c.sort_order)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("index {} {} {}", c.doc, c.section, c.lang))?;
    }
    let docs: Vec<String> = chunks.iter().map(|c| c.doc.to_string()).collect();
    let sections: Vec<String> = chunks.iter().map(|c| c.section.clone()).collect();
    let parts: Vec<i16> = chunks.iter().map(|c| c.part).collect();
    let langs: Vec<String> = chunks.iter().map(|c| c.lang.to_string()).collect();
    sqlx::query(
        "DELETE FROM handbook_chunks h WHERE NOT EXISTS (
           SELECT 1 FROM unnest($1::text[], $2::text[], $3::int2[], $4::text[]) AS k(doc, section, part, lang)
           WHERE k.doc = h.doc AND k.section = h.section AND k.part = h.part AND k.lang = h.lang)",
    )
    .bind(&docs)
    .bind(&sections)
    .bind(&parts)
    .bind(&langs)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let embedded = embed_missing(db, ai, settings).await;
    let without_vector: i64 = sqlx::query_scalar("SELECT count(*) FROM handbook_chunks WHERE embedding IS NULL OR embed_model IS DISTINCT FROM $1")
        .bind(&settings.embed_model)
        .fetch_one(db)
        .await?;
    Ok(SyncReport { chunks: chunks.len(), embedded, without_vector: without_vector as usize })
}

#[derive(sqlx::FromRow)]
struct Pending {
    id: Uuid,
    chapter: String,
    title: String,
    content: String,
}

async fn embed_missing(db: &PgPool, ai: &Ai, settings: &AiSettings) -> usize {
    let pending = match sqlx::query_as::<_, Pending>(
        "SELECT id, chapter, title, content FROM handbook_chunks WHERE embedding IS NULL OR embed_model IS DISTINCT FROM $1 ORDER BY sort_order, part, lang",
    )
    .bind(&settings.embed_model)
    .fetch_all(db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!(error = %e, "handbook: could not list chunks to embed");
            return 0;
        }
    };
    let mut done = 0;
    for p in pending {
        if settings::check_budget(db, settings).await.is_err() {
            tracing::warn!("handbook: monthly budget reached; remaining chunks stay keyword-only");
            break;
        }
        let text = document_text(&p.chapter, &p.title, &p.content);
        let e = match ai.embed(&settings.embed_model, &text).await {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(error = ?e, "handbook: embedding failed; search uses keywords until the next start");
                break;
            }
        };
        settings::record_usage(
            db,
            settings,
            UsageRecord { purpose: "embed", model: &settings.embed_model, prompt_tokens: e.prompt_tokens, completion_tokens: 0, user_id: None, run_id: None },
        )
        .await;
        let r = sqlx::query("UPDATE handbook_chunks SET embedding = $2::vector, embed_model = $3 WHERE id = $1")
            .bind(p.id)
            .bind(vector_literal(&e.vector))
            .bind(&settings.embed_model)
            .execute(db)
            .await;
        if let Err(e) = r {
            tracing::warn!(error = %e, "handbook: could not store an embedding");
            break;
        }
        done += 1;
    }
    done
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

/// Who is reading; decides which `visibility` values count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    /// MCP and anyone without an account: `public` only.
    Public,
    /// Signed-in trainers: `public` and `trainer`.
    Trainer,
    /// Staff and admins: everything.
    Staff,
}

impl Audience {
    pub fn visibilities(self) -> Vec<String> {
        let v: &[&str] = match self {
            Self::Public => &["public"],
            Self::Trainer => &["public", "trainer"],
            Self::Staff => &["public", "trainer", "staff"],
        };
        v.iter().map(|s| s.to_string()).collect()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    /// `handbook` or `academy_info`.
    pub doc: String,
    pub section: String,
    pub chapter: String,
    pub title: String,
    pub content: String,
    /// Language of `content`; may differ from the reader's when no translation exists yet.
    pub lang: String,
    /// Cosine distance (vector mode) or 1 − keyword score (keyword mode); smaller is closer.
    pub distance: f64,
    pub visibility: String,
    pub source_type: String,
    pub source_ref: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchOutcome {
    pub found: bool,
    /// `vector`, or `keyword` when embeddings are unavailable.
    pub method: &'static str,
    pub results: Vec<Hit>,
    /// Embedding tokens spent on the query; the MCP server records them itself.
    #[serde(skip)]
    pub embed_tokens: i64,
}

pub struct SearchRequest<'a> {
    pub query: &'a str,
    pub top_k: usize,
    pub locale: Locale,
    pub audience: Audience,
    pub user_id: Option<Uuid>,
    /// `rag` for the agent and the manual page, `mcp_embed` for the MCP server.
    pub purpose: &'static str,
    /// Only `eval-rag` sets this, to see the ranking past the usual cut-off.
    pub max_distance: Option<f64>,
    /// `handbook_chunks`, or the public-only `mcp_handbook` view for the MCP server.
    pub source: Source,
    /// False forces keyword mode (for example when the MCP budget is used up).
    pub allow_embed: bool,
}

/// Where a search reads from. The MCP server reads the public view and meters its own spending,
/// so its database role needs no access to the full tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Main,
    PublicView,
}

impl Source {
    fn table(self) -> &'static str {
        match self {
            Self::Main => "handbook_chunks",
            Self::PublicView => "mcp_handbook",
        }
    }
}

#[derive(sqlx::FromRow, Clone)]
struct Candidate {
    doc: String,
    section: String,
    lang: String,
    chapter: String,
    title: String,
    content: String,
    visibility: String,
    source_type: String,
    source_ref: String,
    distance: f64,
}

/// Search the handbook and academy information. Only chunks the audience may read are considered.
pub async fn search(db: &PgPool, ai: &Ai, settings: &AiSettings, req: &SearchRequest<'_>) -> Result<SearchOutcome> {
    let query = req.query.trim();
    if query.is_empty() {
        bail!("query is empty");
    }
    let top_k = req.top_k.clamp(1, MAX_TOP_K);
    let visibilities = req.audience.visibilities();

    let table = req.source.table();
    let metered_here = req.source == Source::Main;
    let vectors: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table} WHERE embed_model = $1 AND embedding IS NOT NULL")))
        .bind(&settings.embed_model)
        .fetch_one(db)
        .await?;
    let mut embedded = None;
    let mut embed_tokens = 0;
    if vectors > 0 && req.allow_embed && (!metered_here || settings::check_budget(db, settings).await.is_ok()) {
        match ai.embed(&settings.embed_model, &query_text(query)).await {
            Ok(e) => {
                if metered_here {
                    settings::record_usage(
                        db,
                        settings,
                        UsageRecord { purpose: req.purpose, model: &settings.embed_model, prompt_tokens: e.prompt_tokens, completion_tokens: 0, user_id: req.user_id, run_id: None },
                    )
                    .await;
                }
                embed_tokens = e.prompt_tokens;
                embedded = Some(e.vector);
            }
            Err(e) => tracing::warn!(error = ?e, "handbook: query embedding failed; using keywords"),
        }
    }

    let (method, candidates, limit) = match embedded {
        Some(vector) => {
            let rows = sqlx::query_as::<_, Candidate>(sqlx::AssertSqlSafe(format!(
                "SELECT doc, section, lang, chapter, title, content, visibility, source_type, source_ref,
                        (embedding <=> $1::vector)::float8 AS distance
                 FROM {table}
                 WHERE embed_model = $2 AND embedding IS NOT NULL AND visibility = ANY($3)
                 ORDER BY embedding <=> $1::vector LIMIT $4"
            )))
            .bind(vector_literal(&vector))
            .bind(&settings.embed_model)
            .bind(&visibilities)
            .bind(CANDIDATES)
            .fetch_all(db)
            .await?;
            // Hybrid ranking: shared wording pulls a chunk closer, so a section titled 採訪申請 wins
            // a question about 採訪 even when the embedding puts a paperwork section first.
            let mut rows: Vec<Candidate> = rows
                .into_iter()
                .map(|mut c| {
                    c.distance -= KEYWORD_WEIGHT * keyword_score(query, &format!("{} {}", c.title, c.content));
                    c
                })
                .collect();
            rows.sort_by(|a, b| a.distance.total_cmp(&b.distance));
            ("vector", rows, req.max_distance.unwrap_or_else(|| max_distance(&settings.embed_model)))
        }
        None => {
            let rows = sqlx::query_as::<_, Candidate>(sqlx::AssertSqlSafe(format!(
                "SELECT doc, section, lang, chapter, title, content, visibility, source_type, source_ref, 1.0::float8 AS distance
                 FROM {table} WHERE visibility = ANY($1)"
            )))
            .bind(&visibilities)
            .fetch_all(db)
            .await?;
            let mut scored: Vec<Candidate> = rows
                .into_iter()
                .map(|mut c| {
                    c.distance = 1.0 - keyword_score(query, &format!("{} {}", c.title, c.content));
                    c
                })
                .collect();
            scored.sort_by(|a, b| a.distance.total_cmp(&b.distance));
            ("keyword", scored, req.max_distance.unwrap_or(1.0 - KEYWORD_MIN_SCORE))
        }
    };

    // Best chunk per section, closest first.
    let mut best: Vec<Candidate> = Vec::new();
    let mut seen = HashSet::new();
    for c in candidates {
        if c.distance > limit {
            break;
        }
        if seen.insert((c.doc.clone(), c.section.clone())) {
            best.push(c);
        }
        if best.len() == top_k {
            break;
        }
    }

    let mut results = Vec::new();
    for c in best {
        results.push(whole_section(db, &c, req.locale, &visibilities, table).await?);
    }
    Ok(SearchOutcome { found: !results.is_empty(), method, results, embed_tokens })
}

#[derive(sqlx::FromRow)]
struct Part {
    lang: String,
    chapter: String,
    title: String,
    content: String,
}

/// The whole section in the reader's language, else Traditional Chinese, else what matched.
async fn whole_section(db: &PgPool, c: &Candidate, locale: Locale, visibilities: &[String], table: &'static str) -> Result<Hit> {
    let parts = sqlx::query_as::<_, Part>(sqlx::AssertSqlSafe(format!(
        "SELECT lang, chapter, title, content FROM {table}
         WHERE doc = $1 AND section = $2 AND visibility = ANY($3) ORDER BY part"
    )))
    .bind(&c.doc)
    .bind(&c.section)
    .bind(visibilities)
    .fetch_all(db)
    .await?;
    let mut by_lang: HashMap<String, Vec<Part>> = HashMap::new();
    for p in parts {
        by_lang.entry(p.lang.clone()).or_default().push(p);
    }
    let pick = [locale.code(), "zh-TW", c.lang.as_str()].into_iter().find(|l| by_lang.contains_key(*l)).map(str::to_string);
    let (lang, chapter, title, content) = match pick.and_then(|l| by_lang.remove(&l).map(|p| (l, p))) {
        Some((lang, parts)) => {
            let chapter = parts[0].chapter.clone();
            let title = parts[0].title.clone();
            let content = parts.into_iter().map(|p| p.content).collect::<Vec<_>>().join("\n\n");
            (lang, chapter, title, content)
        }
        None => (c.lang.clone(), c.chapter.clone(), c.title.clone(), c.content.clone()),
    };
    Ok(Hit {
        doc: c.doc.clone(),
        section: c.section.clone(),
        chapter,
        title,
        content,
        lang,
        distance: (c.distance * 1000.0).round() / 1000.0,
        visibility: c.visibility.clone(),
        source_type: c.source_type.clone(),
        source_ref: c.source_ref.clone(),
    })
}

/// Character pairs of the query found in `text`, as a share of all of them (0–1). Works for
/// Chinese and Japanese without a word splitter; Latin text is compared in lower case.
pub fn keyword_score(query: &str, text: &str) -> f64 {
    let pairs = |s: &str| -> HashSet<(char, char)> {
        let chars: Vec<char> = s.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect();
        chars.windows(2).map(|w| (w[0], w[1])).collect()
    };
    let q = pairs(query);
    if q.is_empty() {
        return 0.0;
    }
    let t = pairs(text);
    q.iter().filter(|p| t.contains(p)).count() as f64 / q.len() as f64
}

/// The table of contents and full text for the manual page, in the reader's language where it exists.
#[derive(Debug, Serialize)]
pub struct Section {
    pub section: String,
    pub chapter: String,
    pub title: String,
    pub content: String,
    pub lang: String,
    pub visibility: String,
}

pub async fn sections(db: &PgPool, locale: Locale, audience: Audience) -> sqlx::Result<Vec<Section>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        section: String,
        lang: String,
        chapter: String,
        title: String,
        content: String,
        visibility: String,
    }
    let rows = sqlx::query_as::<_, Row>(
        "SELECT section, lang, chapter, title, content, visibility FROM handbook_chunks
         WHERE doc = 'handbook' AND visibility = ANY($1) ORDER BY sort_order, part",
    )
    .bind(audience.visibilities())
    .fetch_all(db)
    .await?;
    let mut order: Vec<String> = Vec::new();
    let mut by_section: HashMap<String, BTreeMap<String, Section>> = HashMap::new();
    for r in rows {
        if !by_section.contains_key(&r.section) {
            order.push(r.section.clone());
        }
        let langs = by_section.entry(r.section.clone()).or_default();
        match langs.get_mut(&r.lang) {
            Some(s) => {
                s.content.push_str("\n\n");
                s.content.push_str(&r.content);
            }
            None => {
                langs.insert(
                    r.lang.clone(),
                    Section { section: r.section, chapter: r.chapter, title: r.title, content: r.content, lang: r.lang, visibility: r.visibility },
                );
            }
        }
    }
    Ok(order
        .into_iter()
        .filter_map(|id| {
            let mut langs = by_section.remove(&id)?;
            let key = [locale.code(), "zh-TW"].into_iter().find(|l| langs.contains_key(*l)).map(str::to_string).or_else(|| langs.keys().next().cloned())?;
            langs.remove(&key)
        })
        .collect())
}

/// Offline embedding check used by tests: related text must sit closer than unrelated text.
#[cfg(test)]
fn mock_distance(a: &str, b: &str) -> f64 {
    let (x, y) = (crate::llm::mock::embed(a), crate::llm::mock::embed(b));
    let dot: f32 = x.iter().zip(&y).map(|(p, q)| p * q).sum();
    let norm = |v: &[f32]| v.iter().map(|p| p * p).sum::<f32>().sqrt();
    1.0 - f64::from(dot / (norm(&x) * norm(&y)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Title\n<!-- header note -->\n## 第 1 章 總則\n\n### §1.1 目的\n<!-- visibility: public -->\n第一段。\n\n第二段。\n\n### §1.2 名詞\n內容。\n";

    #[test]
    fn parses_sections_chapters_and_visibility() {
        let chunks = parse_handbook(SAMPLE, "zh-TW").unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].section, "§1.1");
        assert_eq!(chunks[0].chapter, "第 1 章 總則");
        assert_eq!(chunks[0].visibility, "public");
        assert_eq!(chunks[0].content, "第一段。\n\n第二段。");
        assert_eq!(chunks[1].visibility, "trainer");
        assert!(!chunks[0].content.contains("header note"));
    }

    #[test]
    fn rejects_bad_headings_duplicates_and_empty_sections() {
        assert!(parse_handbook("### 3.2 no mark\ntext\n", "zh-TW").is_err());
        assert!(parse_handbook("### §1.1 a\nx\n### §1.1 b\ny\n", "zh-TW").is_err());
        assert!(parse_handbook("### §1.1 a\n<!-- visibility: everyone -->\nx\n", "zh-TW").is_err());
        assert!(parse_handbook("### §1.1 a\n\n### §1.2 b\ny\n", "zh-TW").is_err());
    }

    #[test]
    fn long_sections_split_at_paragraphs() {
        let para = "字".repeat(500);
        let text = format!("### §1.1 長\n{para}\n\n{para}\n\n{para}\n");
        let chunks = parse_handbook(&text, "zh-TW").unwrap();
        assert_eq!(chunks.len(), 3);
        assert!(chunks.iter().all(|c| c.section == "§1.1"));
        assert_eq!(chunks.iter().map(|c| c.part).collect::<Vec<_>>(), vec![0, 1, 2]);
    }

    #[test]
    fn same_text_same_hash() {
        let a = &parse_handbook(SAMPLE, "zh-TW").unwrap()[0];
        let mut b = a.clone();
        assert_eq!(a.hash(), b.hash());
        b.visibility = "trainer".into();
        assert_eq!(a.hash(), b.hash(), "visibility changes must not force a new embedding");
        b.content.push('！');
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn the_real_handbook_parses() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/rag");
        let chunks = load_files(&dir).expect("data/rag handbook files are valid");
        let sections: HashSet<_> = chunks.iter().filter(|c| c.lang == "zh-TW").map(|c| c.section.clone()).collect();
        assert!(sections.len() >= 40, "the handbook needs at least 40 sections, found {}", sections.len());
        assert!(chunks.iter().all(|c| !c.content.contains("要寫：")), "outline notes left in the handbook");
    }

    #[test]
    fn parity_catches_missing_sections() {
        let zh = parse_handbook("### §1.1 a\nx\n### §1.2 b\ny\n", "zh-TW").unwrap();
        let en = parse_handbook("### §1.1 a\nx\n", "en-GB").unwrap();
        let map = BTreeMap::from([("zh-TW", zh), ("en-GB", en)]);
        assert!(check_parity(&map).is_err());
    }

    #[test]
    fn keyword_score_finds_overlap() {
        assert!(keyword_score("報支期限", "報支要在返校後 14 天內，把單據送到秘書室。報支期限") > 0.9);
        assert!(keyword_score("停車位", "報支要在返校後 14 天內") < 0.2);
    }

    #[test]
    fn mock_embedding_ranks_related_text_first() {
        assert!(mock_distance("遠征的交通費怎麼報支", "交通與住宿費報支的範圍") < mock_distance("遠征的交通費怎麼報支", "宿舍訪客登記"));
    }

    #[test]
    fn audiences_only_widen() {
        assert_eq!(Audience::Public.visibilities(), vec!["public"]);
        assert!(Audience::Trainer.visibilities().contains(&"trainer".to_string()));
        assert!(!Audience::Trainer.visibilities().contains(&"staff".to_string()));
    }
}
