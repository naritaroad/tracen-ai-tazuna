//! Academy catalog: information topics, the academy calendar and training services.
//! The AI tools and the manual pages both call these functions.

pub mod http;
pub mod matching;
pub mod plan;

use serde::Serialize;
use sqlx::PgPool;
use time::macros::offset;
use time::{Date, Month, OffsetDateTime, UtcOffset};

use crate::locale::Locale;

/// Tracen Academy is in Japan (no daylight saving time).
pub const ACADEMY_OFFSET: UtcOffset = offset!(+9);

pub fn academy_now() -> OffsetDateTime {
    OffsetDateTime::now_utc().to_offset(ACADEMY_OFFSET)
}

// ---------------------------------------------------------------------------
// Information topics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InfoRow {
    pub topic: String,
    pub title: String,
    pub title_ja: String,
    pub title_en: String,
    pub content: String,
    pub content_ja: String,
    pub content_en: String,
    pub source_type: String,
    pub source_ref: String,
    pub updated_on: Date,
}

#[derive(Debug, Serialize)]
pub struct InfoView {
    pub topic: String,
    pub title: String,
    pub content: String,
    pub source_type: String,
    pub source_ref: String,
    pub updated_on: String,
}

impl InfoRow {
    pub fn view(&self, locale: Locale) -> InfoView {
        InfoView {
            topic: self.topic.clone(),
            title: locale.pick(&self.title, &self.title_ja, &self.title_en).to_string(),
            content: locale.pick(&self.content, &self.content_ja, &self.content_en).to_string(),
            source_type: self.source_type.clone(),
            source_ref: self.source_ref.clone(),
            updated_on: self.updated_on.to_string(),
        }
    }
}

pub async fn list_info(db: &PgPool) -> sqlx::Result<Vec<InfoRow>> {
    sqlx::query_as::<_, InfoRow>(
        "SELECT topic, title, title_ja, title_en, content, content_ja, content_en, source_type, source_ref, updated_on
         FROM academy_info ORDER BY sort_order, topic",
    )
    .fetch_all(db)
    .await
}

pub async fn get_info(db: &PgPool, topic: &str) -> sqlx::Result<Option<InfoRow>> {
    sqlx::query_as::<_, InfoRow>(
        "SELECT topic, title, title_ja, title_en, content, content_ja, content_en, source_type, source_ref, updated_on
         FROM academy_info WHERE topic = lower($1)",
    )
    .bind(topic.trim())
    .fetch_optional(db)
    .await
}

pub async fn info_topics(db: &PgPool) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar("SELECT topic FROM academy_info ORDER BY sort_order, topic").fetch_all(db).await
}

// ---------------------------------------------------------------------------
// Academy calendar
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EventRow {
    pub id: String,
    pub title: String,
    pub title_ja: String,
    pub title_en: String,
    pub kind: String,
    pub starts_at: OffsetDateTime,
    pub ends_at: OffsetDateTime,
    pub affects: Vec<String>,
    pub source_type: String,
    /// Set on the entries made from the race schedule (`kind = "race"`).
    #[sqlx(skip)]
    pub race: Option<RaceView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventView {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub starts_at: String,
    pub ends_at: String,
    pub affects_services: Vec<String>,
    pub source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub race: Option<RaceView>,
}

impl EventRow {
    pub fn view(&self, locale: Locale) -> EventView {
        EventView {
            id: self.id.clone(),
            title: locale.pick(&self.title, &self.title_ja, &self.title_en).to_string(),
            kind: self.kind.clone(),
            starts_at: format_local(self.starts_at),
            ends_at: format_local(self.ends_at),
            affects_services: self.affects.clone(),
            source_type: self.source_type.clone(),
            race: self.race.clone(),
        }
    }

    pub fn blocks(&self, service_id: &str) -> bool {
        self.affects.iter().any(|a| a == "all" || a == service_id)
    }
}

/// `2026-10-29 13:00` in academy time.
pub fn format_local(t: OffsetDateTime) -> String {
    let t = t.to_offset(ACADEMY_OFFSET);
    format!("{} {:02}:{:02}", t.date(), t.hour(), t.minute())
}

/// Events and races that overlap the academy-time days `from..=to`.
pub async fn list_events(db: &PgPool, from: Date, to: Date) -> sqlx::Result<Vec<EventRow>> {
    let start = from.midnight().assume_offset(ACADEMY_OFFSET);
    let end = to.next_day().unwrap_or(to).midnight().assume_offset(ACADEMY_OFFSET);
    let mut rows = sqlx::query_as::<_, EventRow>(
        "SELECT id, title, title_ja, title_en, kind, starts_at, ends_at, affects, source_type
         FROM academy_events WHERE starts_at < $2 AND ends_at > $1 ORDER BY starts_at, id",
    )
    .bind(start)
    .bind(end)
    .fetch_all(db)
    .await?;
    let races = sqlx::query_as::<_, RaceRow>(
        "SELECT id, name, name_ja, name_en, grade, racecourse, surface, distance_m, direction, month, half, classes, source_type, source_ref
         FROM races ORDER BY sort_order, id",
    )
    .fetch_all(db)
    .await?;
    rows.extend(race_entries(&races, from, to));
    // Same start: academy events first, then races by grade (G1 before G2).
    let grade = |r: &EventRow| r.race.as_ref().map(|race| race.grade.clone()).unwrap_or_default();
    rows.sort_by(|a, b| a.starts_at.cmp(&b.starts_at).then_with(|| grade(a).cmp(&grade(b))).then_with(|| a.id.cmp(&b.id)));
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Race schedule (game data placed on the calendar every year)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RaceRow {
    pub id: String,
    pub name: String,
    pub name_ja: String,
    pub name_en: String,
    pub grade: String,
    pub racecourse: String,
    pub surface: String,
    pub distance_m: i32,
    pub direction: String,
    pub month: i16,
    pub half: i16,
    pub classes: Vec<String>,
    pub source_type: String,
    pub source_ref: String,
}

/// Race details for the calendar; names of racecourses, surfaces and classes are translated by the client.
#[derive(Debug, Clone, Serialize)]
pub struct RaceView {
    pub grade: String,
    pub racecourse: String,
    pub surface: String,
    pub distance_m: i32,
    pub direction: String,
    pub classes: Vec<String>,
    pub source_ref: String,
}

impl RaceRow {
    /// The race's half month in `year`: the 1st–15th or the 16th–last day, in academy time.
    fn span(&self, year: i32) -> Option<(OffsetDateTime, OffsetDateTime)> {
        let month = Month::try_from(u8::try_from(self.month).ok()?).ok()?;
        let (first, last) = if self.half == 1 { (1, 15) } else { (16, month.length(year)) };
        let starts = Date::from_calendar_date(year, month, first).ok()?.midnight().assume_offset(ACADEMY_OFFSET);
        let ends = Date::from_calendar_date(year, month, last).ok()?.with_hms(23, 59, 0).ok()?.assume_offset(ACADEMY_OFFSET);
        Some((starts, ends))
    }

    fn entry(&self, year: i32) -> Option<EventRow> {
        let (starts_at, ends_at) = self.span(year)?;
        Some(EventRow {
            id: format!("{}_{year}", self.id),
            title: self.name.clone(),
            title_ja: self.name_ja.clone(),
            title_en: self.name_en.clone(),
            kind: "race".into(),
            starts_at,
            ends_at,
            // Races are listed for planning; they never close a facility.
            affects: Vec::new(),
            source_type: self.source_type.clone(),
            race: Some(RaceView {
                grade: self.grade.clone(),
                racecourse: self.racecourse.clone(),
                surface: self.surface.clone(),
                distance_m: self.distance_m,
                direction: self.direction.clone(),
                classes: self.classes.clone(),
                source_ref: self.source_ref.clone(),
            }),
        })
    }
}

/// Calendar entries for every race whose half month overlaps the academy-time days `from..=to`.
pub fn race_entries(races: &[RaceRow], from: Date, to: Date) -> Vec<EventRow> {
    let start = from.midnight().assume_offset(ACADEMY_OFFSET);
    let end = to.next_day().unwrap_or(to).midnight().assume_offset(ACADEMY_OFFSET);
    (from.year()..=to.year())
        .flat_map(|year| races.iter().filter_map(move |r| r.entry(year)))
        .filter(|e| e.starts_at < end && e.ends_at > start)
        .collect()
}

// ---------------------------------------------------------------------------
// Training services (the records table)
// ---------------------------------------------------------------------------

pub const SERVICE_CATEGORIES: [&str; 6] = ["track", "equipment", "analysis", "room", "workshop", "support"];

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ServiceRow {
    pub id: String,
    pub name: String,
    pub name_ja: String,
    pub name_en: String,
    pub category: String,
    pub unit_minutes: i32,
    pub capacity: i32,
    pub fee_jpy: i32,
    pub available: bool,
    pub location: String,
    pub aliases: Vec<String>,
    pub notes: String,
    pub source_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServiceView {
    pub id: String,
    pub name: String,
    pub category: String,
    pub unit_minutes: i32,
    pub capacity: i32,
    pub fee_jpy: i32,
    pub available: bool,
    pub location: String,
    pub notes: String,
    pub source_type: String,
}

impl ServiceRow {
    pub fn localized_name(&self, locale: Locale) -> &str {
        locale.pick(&self.name, &self.name_ja, &self.name_en)
    }

    pub fn view(&self, locale: Locale) -> ServiceView {
        ServiceView {
            id: self.id.clone(),
            name: self.localized_name(locale).to_string(),
            category: self.category.clone(),
            unit_minutes: self.unit_minutes,
            capacity: self.capacity,
            fee_jpy: self.fee_jpy,
            available: self.available,
            location: self.location.clone(),
            notes: self.notes.clone(),
            source_type: self.source_type.clone(),
        }
    }
}

pub async fn list_services(db: &PgPool) -> sqlx::Result<Vec<ServiceRow>> {
    sqlx::query_as::<_, ServiceRow>(
        "SELECT id, name, name_ja, name_en, category, unit_minutes, capacity, fee_jpy, available, location, aliases, notes, source_type
         FROM training_services ORDER BY sort_order, id",
    )
    .fetch_all(db)
    .await
}

#[derive(Debug, Default, Clone, serde::Deserialize)]
pub struct ServiceFilter {
    pub query: Option<String>,
    pub category: Option<String>,
    pub only_available: Option<bool>,
}

/// Search by forgiving name match, category and availability. Best matches first.
pub async fn search_services(db: &PgPool, filter: &ServiceFilter) -> sqlx::Result<Vec<ServiceRow>> {
    let rows = list_services(db).await?;
    let rows = rows
        .into_iter()
        .filter(|r| filter.category.as_deref().is_none_or(|c| c.is_empty() || r.category == c))
        .filter(|r| !filter.only_available.unwrap_or(false) || r.available);
    Ok(match filter.query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        None => rows.collect(),
        Some(q) => matching::rank(q, rows.collect()).into_iter().map(|(row, _)| row).collect(),
    })
}

#[cfg(test)]
mod tests {
    use time::macros::{date, datetime};

    use super::*;

    fn race(id: &str, month: i16, half: i16) -> RaceRow {
        RaceRow {
            id: id.into(),
            name: id.into(),
            name_ja: id.into(),
            name_en: id.into(),
            grade: "G1".into(),
            racecourse: "tokyo".into(),
            surface: "turf".into(),
            distance_m: 2400,
            direction: "left".into(),
            month,
            half,
            classes: vec!["classic".into(), "senior".into()],
            source_type: "community".into(),
            source_ref: String::new(),
        }
    }

    #[test]
    fn races_take_their_half_month_every_year() {
        let races = [race("race_japan_cup", 11, 2), race("race_kawasaki_kinen", 2, 1), race("race_february_stakes", 2, 2)];
        let rows = race_entries(&races, date!(2026 - 10 - 26), date!(2027 - 02 - 28));
        let ids: Vec<_> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["race_japan_cup_2026", "race_kawasaki_kinen_2027", "race_february_stakes_2027"]);
        assert_eq!(rows[0].starts_at, datetime!(2026-11-16 00:00 +9));
        assert_eq!(rows[0].ends_at, datetime!(2026-11-30 23:59 +9));
        assert_eq!(rows[1].ends_at, datetime!(2027-02-15 23:59 +9));
        assert_eq!(rows[2].ends_at, datetime!(2027-02-28 23:59 +9));
        assert!(rows.iter().all(|r| r.kind == "race" && r.affects.is_empty() && r.race.is_some()));
    }

    #[test]
    fn races_outside_the_range_are_left_out() {
        let races = [race("race_arima_kinen", 12, 2), race("race_mile_championship", 11, 2)];
        let rows = race_entries(&races, date!(2026 - 11 - 01), date!(2026 - 11 - 15));
        assert!(rows.is_empty());
        let rows = race_entries(&races, date!(2026 - 11 - 15), date!(2026 - 11 - 16));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "race_mile_championship_2026");
    }
}
