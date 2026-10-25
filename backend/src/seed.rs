//! Startup data: the demo catalog from `data/seed/*.csv`, and optional demo accounts.
//! Only catalog tables are rebuilt; accounts, conversations and cases are never cleared.

use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use sqlx::PgPool;
use time::macros::format_description;
use time::{Date, OffsetDateTime};

use crate::auth;
use crate::config::{Config, SeedMode};

#[derive(Deserialize)]
struct InfoCsv {
    topic: String,
    title: String,
    title_ja: String,
    title_en: String,
    content: String,
    content_ja: String,
    content_en: String,
    source_type: String,
    source_ref: String,
    updated_on: String,
}

#[derive(Deserialize)]
struct ServiceCsv {
    id: String,
    name: String,
    name_ja: String,
    name_en: String,
    category: String,
    unit_minutes: i32,
    capacity: i32,
    fee_jpy: i32,
    available: u8,
    location: String,
    aliases: String,
    notes: String,
    source_type: String,
}

#[derive(Deserialize)]
struct EventCsv {
    id: String,
    title: String,
    title_ja: String,
    title_en: String,
    kind: String,
    starts_at: String,
    ends_at: String,
    affects: String,
    source_type: String,
}

#[derive(Deserialize)]
struct RaceCsv {
    id: String,
    name: String,
    name_ja: String,
    name_en: String,
    grade: String,
    racecourse: String,
    surface: String,
    distance_m: i32,
    direction: String,
    month: i16,
    half: i16,
    classes: String,
    source_type: String,
    source_ref: String,
}

fn read_csv<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>> {
    let mut reader = csv::Reader::from_path(path).with_context(|| format!("open {}", path.display()))?;
    reader
        .deserialize()
        .enumerate()
        .map(|(i, row)| row.with_context(|| format!("{} row {}", path.display(), i + 2)))
        .collect()
}

fn split_list(text: &str) -> Vec<String> {
    text.split('|').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect()
}

pub struct SeedReport {
    pub topics: usize,
    pub services: usize,
    pub events: usize,
    pub races: usize,
    pub skipped: bool,
}

/// Rebuild (or fill, in `if_empty` mode) the catalog tables in one transaction.
pub async fn import_catalog(db: &PgPool, dir: &Path, mode: SeedMode) -> Result<SeedReport> {
    if mode == SeedMode::IfEmpty {
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM academy_info").fetch_one(db).await?;
        if count > 0 {
            return Ok(SeedReport { topics: 0, services: 0, events: 0, races: 0, skipped: true });
        }
    }

    let info: Vec<InfoCsv> = read_csv(&dir.join("academy_info.csv"))?;
    let services: Vec<ServiceCsv> = read_csv(&dir.join("training_services.csv"))?;
    let events: Vec<EventCsv> = read_csv(&dir.join("academy_events.csv"))?;
    let races: Vec<RaceCsv> = read_csv(&dir.join("race_schedule.csv"))?;
    if info.len() < 8 {
        bail!("academy_info.csv needs at least 8 topics, found {}", info.len());
    }
    if services.len() < 15 {
        bail!("training_services.csv needs at least 15 rows, found {}", services.len());
    }

    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM academy_info").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM training_services").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM academy_events").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM races").execute(&mut *tx).await?;

    for (i, r) in info.iter().enumerate() {
        let updated = Date::parse(&r.updated_on, format_description!("[year]-[month]-[day]"))
            .with_context(|| format!("academy_info {}: updated_on", r.topic))?;
        sqlx::query(
            "INSERT INTO academy_info (topic, title, title_ja, title_en, content, content_ja, content_en, source_type, source_ref, updated_on, sort_order)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(r.topic.trim().to_lowercase())
        .bind(&r.title)
        .bind(&r.title_ja)
        .bind(&r.title_en)
        .bind(&r.content)
        .bind(&r.content_ja)
        .bind(&r.content_en)
        .bind(&r.source_type)
        .bind(&r.source_ref)
        .bind(updated)
        .bind(i as i32)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("insert academy_info {}", r.topic))?;
    }

    for (i, r) in services.iter().enumerate() {
        sqlx::query(
            "INSERT INTO training_services (id, name, name_ja, name_en, category, unit_minutes, capacity, fee_jpy, available, location, aliases, notes, source_type, sort_order)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)",
        )
        .bind(&r.id)
        .bind(&r.name)
        .bind(&r.name_ja)
        .bind(&r.name_en)
        .bind(&r.category)
        .bind(r.unit_minutes)
        .bind(r.capacity)
        .bind(r.fee_jpy)
        .bind(r.available == 1)
        .bind(&r.location)
        .bind(split_list(&r.aliases))
        .bind(&r.notes)
        .bind(&r.source_type)
        .bind(i as i32)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("insert training_services {}", r.id))?;
    }

    let rfc3339 = &time::format_description::well_known::Rfc3339;
    for r in &events {
        let starts = OffsetDateTime::parse(&r.starts_at, rfc3339).with_context(|| format!("academy_events {}: starts_at", r.id))?;
        let ends = OffsetDateTime::parse(&r.ends_at, rfc3339).with_context(|| format!("academy_events {}: ends_at", r.id))?;
        sqlx::query(
            "INSERT INTO academy_events (id, title, title_ja, title_en, kind, starts_at, ends_at, affects, source_type)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(&r.id)
        .bind(&r.title)
        .bind(&r.title_ja)
        .bind(&r.title_en)
        .bind(&r.kind)
        .bind(starts)
        .bind(ends)
        .bind(split_list(&r.affects))
        .bind(&r.source_type)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("insert academy_events {}", r.id))?;
    }

    for (i, r) in races.iter().enumerate() {
        sqlx::query(
            "INSERT INTO races (id, name, name_ja, name_en, grade, racecourse, surface, distance_m, direction, month, half, classes, source_type, source_ref, sort_order)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
        )
        .bind(&r.id)
        .bind(&r.name)
        .bind(&r.name_ja)
        .bind(&r.name_en)
        .bind(&r.grade)
        .bind(&r.racecourse)
        .bind(&r.surface)
        .bind(r.distance_m)
        .bind(&r.direction)
        .bind(r.month)
        .bind(r.half)
        .bind(split_list(&r.classes))
        .bind(&r.source_type)
        .bind(&r.source_ref)
        .bind(i as i32)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("insert races {}", r.id))?;
    }

    tx.commit().await?;
    Ok(SeedReport { topics: info.len(), services: services.len(), events: events.len(), races: races.len(), skipped: false })
}

/// Demo accounts for development. Passwords come from the environment.
pub async fn ensure_demo_accounts(db: &PgPool, config: &Config) -> Result<()> {
    let Some(demo) = &config.demo_accounts else {
        return Ok(());
    };
    let accounts = [
        ("trainer", "見習い訓練員", "trainer", &demo.trainer_password),
        ("trainer2", "第二訓練員", "trainer", &demo.trainer_password),
        ("director", "秋川理事長（真人服務）", "staff", &demo.staff_password),
        ("admin", "系統管理員", "admin", &demo.admin_password),
    ];
    for (username, display_name, role, password) in accounts {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM users WHERE lower(username) = lower($1))")
            .bind(username)
            .fetch_one(db)
            .await?;
        if exists {
            continue;
        }
        let hash = auth::hash_password(password)?;
        sqlx::query("INSERT INTO users (username, display_name, role, password_hash) VALUES ($1, $2, $3, $4)")
            .bind(username)
            .bind(display_name)
            .bind(role)
            .bind(hash)
            .execute(db)
            .await?;
        tracing::info!(username, role, "created demo account");
    }
    Ok(())
}
