//! Training plan calculation: minutes, demo fees, and clashes with the academy calendar.
//! All arithmetic happens here, never in the model.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::macros::format_description;
use time::{Date, Duration, PrimitiveDateTime, Time};

use super::matching::{Resolved, resolve};
use super::{ACADEMY_OFFSET, EventRow, ServiceRow, format_local};
use crate::locale::Locale;

/// Facilities close at 20:30 (`facility_use` topic) so trainees are back before the 21:00 curfew.
pub const LAST_USE: Time = time::macros::time!(20:30);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanItem {
    /// The service in the trainer's own words, or its id.
    pub service: String,
    pub sessions: u32,
    pub minutes_per_session: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanInput {
    pub items: Vec<PlanItem>,
    /// YYYY-MM-DD, academy time.
    pub date: Option<String>,
    /// HH:MM, academy time.
    pub start_time: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PlanLine {
    pub service_id: String,
    pub name: String,
    pub sessions: u32,
    pub minutes_per_session: u32,
    pub minutes: u32,
    pub fee_per_session_jpy: u32,
    pub fee_jpy: u32,
    pub available: bool,
    pub source_type: String,
}

#[derive(Debug, Serialize)]
pub struct Problem {
    pub item: String,
    pub kind: &'static str,
    pub candidates: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Block {
    pub service_id: String,
    pub name: String,
    pub start: String,
    pub end: String,
}

#[derive(Debug, Serialize)]
pub struct Schedule {
    pub date: String,
    pub start: String,
    pub end: String,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Serialize)]
pub struct Conflict {
    /// `event`, `same_day_event`, `service_unavailable` or `after_hours`.
    pub kind: &'static str,
    pub service_id: String,
    pub event_id: Option<String>,
    pub event_title: Option<String>,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct PlanResult {
    /// False when any item is unclear; then there is no total.
    pub complete: bool,
    pub lines: Vec<PlanLine>,
    pub problems: Vec<Problem>,
    pub total_minutes: Option<u32>,
    pub total_fee_jpy: Option<u32>,
    pub formula: Option<String>,
    pub schedule: Option<Schedule>,
    pub conflicts: Vec<Conflict>,
    pub note: String,
}

pub struct Parsed {
    pub date: Option<Date>,
    pub start: Option<Time>,
}

/// Check ranges and formats before touching data.
pub fn validate(input: &PlanInput) -> Result<Parsed, String> {
    if input.items.is_empty() || input.items.len() > 20 {
        return Err("items must have 1 to 20 entries".into());
    }
    for item in &input.items {
        if item.service.trim().is_empty() {
            return Err("every item needs a service".into());
        }
        if !(1..=50).contains(&item.sessions) {
            return Err(format!("sessions for '{}' must be 1 to 50", item.service));
        }
        if let Some(m) = item.minutes_per_session
            && !(5..=480).contains(&m)
        {
            return Err(format!("minutes_per_session for '{}' must be 5 to 480", item.service));
        }
    }
    let date = match input.date.as_deref().filter(|d| !d.is_empty()) {
        None => None,
        Some(d) => Some(
            Date::parse(d, format_description!("[year]-[month]-[day]")).map_err(|_| format!("date '{d}' is not YYYY-MM-DD"))?,
        ),
    };
    let start = match input.start_time.as_deref().filter(|t| !t.is_empty()) {
        None => None,
        Some(t) => Some(Time::parse(t, format_description!("[hour]:[minute]")).map_err(|_| format!("start_time '{t}' is not HH:MM"))?),
    };
    if start.is_some() && date.is_none() {
        return Err("start_time needs a date".into());
    }
    Ok(Parsed { date, start })
}

/// The pure calculation, separated from the database for testing.
pub fn calculate(input: &PlanInput, parsed: &Parsed, services: &[ServiceRow], events: &[EventRow], locale: Locale) -> PlanResult {
    let mut lines = Vec::new();
    let mut problems = Vec::new();
    let mut resolved: Vec<&ServiceRow> = Vec::new();

    for item in &input.items {
        match resolve(&item.service, services) {
            Resolved::One(row) => {
                let row = services.iter().find(|s| s.id == row.id).expect("resolved from this slice");
                let per = item.minutes_per_session.unwrap_or(row.unit_minutes.max(0) as u32);
                let fee = row.fee_jpy.max(0) as u32;
                lines.push(PlanLine {
                    service_id: row.id.clone(),
                    name: row.localized_name(locale).to_string(),
                    sessions: item.sessions,
                    minutes_per_session: per,
                    minutes: per * item.sessions,
                    fee_per_session_jpy: fee,
                    fee_jpy: fee * item.sessions,
                    available: row.available,
                    source_type: row.source_type.clone(),
                });
                resolved.push(row);
            }
            Resolved::Ambiguous(rows) => problems.push(Problem {
                item: item.service.clone(),
                kind: "ambiguous",
                candidates: rows.iter().map(|r| format!("{} ({})", r.localized_name(locale), r.id)).collect(),
            }),
            Resolved::None => problems.push(Problem {
                item: item.service.clone(),
                kind: "not_found",
                candidates: Vec::new(),
            }),
        }
    }

    // A total of only some items is a wrong total.
    if !problems.is_empty() {
        return PlanResult {
            complete: false,
            lines,
            problems,
            total_minutes: None,
            total_fee_jpy: None,
            formula: None,
            schedule: None,
            conflicts: Vec::new(),
            note: "No total yet. Ask the trainer which service they mean, then call the tool again.".into(),
        };
    }

    let total_minutes = lines.iter().map(|l| l.minutes).sum();
    let total_fee = lines.iter().map(|l| l.fee_jpy).sum();
    let minutes_formula =
        lines.iter().map(|l| format!("{} {}min×{}", l.service_id, l.minutes_per_session, l.sessions)).collect::<Vec<_>>().join(" + ");
    let fee_formula =
        lines.iter().map(|l| format!("{}JPY×{}", l.fee_per_session_jpy, l.sessions)).collect::<Vec<_>>().join(" + ");
    let formula = format!("minutes: {minutes_formula} = {total_minutes}; fee: {fee_formula} = {total_fee} JPY");

    let mut conflicts: Vec<Conflict> = lines
        .iter()
        .filter(|l| !l.available)
        .map(|l| Conflict {
            kind: "service_unavailable",
            service_id: l.service_id.clone(),
            event_id: None,
            event_title: None,
            detail: format!("{} is not available now", l.name),
        })
        .collect();

    let schedule = match (parsed.date, parsed.start) {
        (Some(date), Some(start)) => {
            let mut cursor = PrimitiveDateTime::new(date, start).assume_offset(ACADEMY_OFFSET);
            let last_use = PrimitiveDateTime::new(date, LAST_USE).assume_offset(ACADEMY_OFFSET);
            let mut blocks = Vec::new();
            for (line, row) in lines.iter().zip(&resolved) {
                let begin = cursor;
                let end = begin + Duration::minutes(i64::from(line.minutes));
                for ev in events.iter().filter(|e| e.blocks(&row.id) && e.starts_at < end && e.ends_at > begin) {
                    conflicts.push(Conflict {
                        kind: "event",
                        service_id: row.id.clone(),
                        event_id: Some(ev.id.clone()),
                        event_title: Some(locale.pick(&ev.title, &ev.title_ja, &ev.title_en).to_string()),
                        detail: format!("{} – {} overlaps {} – {}", format_local(begin), format_local(end), format_local(ev.starts_at), format_local(ev.ends_at)),
                    });
                }
                if end > last_use {
                    conflicts.push(Conflict {
                        kind: "after_hours",
                        service_id: row.id.clone(),
                        event_id: None,
                        event_title: None,
                        detail: format!("ends at {}, after the 20:30 last use", format_local(end)),
                    });
                }
                blocks.push(Block {
                    service_id: row.id.clone(),
                    name: line.name.clone(),
                    start: format_local(begin),
                    end: format_local(end),
                });
                cursor = end;
            }
            Some(Schedule {
                date: date.to_string(),
                start: blocks.first().map(|b| b.start.clone()).unwrap_or_default(),
                end: format_local(cursor),
                blocks,
            })
        }
        (Some(_), None) => {
            for row in &resolved {
                for ev in events.iter().filter(|e| e.blocks(&row.id)) {
                    conflicts.push(Conflict {
                        kind: "same_day_event",
                        service_id: row.id.clone(),
                        event_id: Some(ev.id.clone()),
                        event_title: Some(locale.pick(&ev.title, &ev.title_ja, &ev.title_en).to_string()),
                        detail: format!("{} – {}", format_local(ev.starts_at), format_local(ev.ends_at)),
                    });
                }
            }
            None
        }
        _ => None,
    };

    let note = if parsed.date.is_some() && parsed.start.is_none() {
        "Give a start_time to check exact overlaps."
    } else {
        "Fees and lengths are demo settings of this system (source_type project)."
    };

    PlanResult {
        complete: true,
        lines,
        problems,
        total_minutes: Some(total_minutes),
        total_fee_jpy: Some(total_fee),
        formula: Some(formula),
        schedule,
        conflicts,
        note: note.into(),
    }
}

/// Load what the calculation needs and run it.
pub async fn calculate_from_db(db: &PgPool, input: &PlanInput, locale: Locale) -> Result<PlanResult, PlanError> {
    let parsed = validate(input).map_err(PlanError::Invalid)?;
    let services = super::list_services(db).await?;
    let events = match parsed.date {
        Some(d) => super::list_events(db, d, d).await?,
        None => Vec::new(),
    };
    Ok(calculate(input, &parsed, &services, &events, locale))
}

#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::matching::tests::service;
    use time::macros::datetime;

    fn services() -> Vec<ServiceRow> {
        let mut stretch = service("stretch", "伸展空間", "ストレッチルーム", "Stretching room", &["伸展"]);
        stretch.unit_minutes = 30;
        let turf = service("turf_basic", "草地基本訓練", "芝コース基礎トレーニング", "Turf course basic training", &["草地"]);
        let mut timing = service("timing", "計時設備租借", "計測機器の貸出", "Timing equipment rental", &["計時"]);
        timing.fee_jpy = 500;
        let mut video = service("video", "訓練影像檢視", "トレーニング映像レビュー", "Training video review", &["影像"]);
        video.fee_jpy = 1000;
        let mut slope = service("slope", "坡道訓練", "坂路トレーニング", "Uphill slope training", &["坂路"]);
        slope.available = false;
        vec![stretch, turf, timing, video, slope]
    }

    fn item(service: &str, sessions: u32) -> PlanItem {
        PlanItem { service: service.into(), sessions, minutes_per_session: None }
    }

    fn run(input: PlanInput, events: &[EventRow]) -> PlanResult {
        let parsed = validate(&input).expect("valid input");
        calculate(&input, &parsed, &services(), events, Locale::ZhTw)
    }

    #[test]
    fn adds_minutes_and_fees_in_code() {
        let r = run(PlanInput { items: vec![item("伸展", 1), item("草地", 5)], date: None, start_time: None }, &[]);
        assert_eq!(r.total_minutes, Some(330));
        let r = run(PlanInput { items: vec![item("計時", 3), item("影像", 2)], date: None, start_time: None }, &[]);
        assert_eq!(r.total_fee_jpy, Some(3500));
    }

    #[test]
    fn no_total_when_an_item_is_unclear() {
        let r = run(PlanInput { items: vec![item("草地", 2), item("swimming pool", 1)], date: None, start_time: None }, &[]);
        assert!(!r.complete);
        assert_eq!(r.total_minutes, None);
        assert_eq!(r.problems[0].kind, "not_found");
    }

    #[test]
    fn finds_calendar_clashes_and_closed_services() {
        let maintenance = EventRow {
            id: "ev_turf_maint_1029".into(),
            title: "第一草地跑道整地".into(),
            title_ja: String::new(),
            title_en: String::new(),
            kind: "maintenance".into(),
            starts_at: datetime!(2026-10-29 13:00 +9),
            ends_at: datetime!(2026-10-29 17:00 +9),
            affects: vec!["turf_basic".into()],
            source_type: "project".into(),
            race: None,
        };
        let input = PlanInput { items: vec![item("草地", 2), item("坂路", 1)], date: Some("2026-10-29".into()), start_time: Some("14:00".into()) };
        let r = run(input, &[maintenance]);
        assert!(r.conflicts.iter().any(|c| c.kind == "event" && c.event_id.as_deref() == Some("ev_turf_maint_1029")));
        assert!(r.conflicts.iter().any(|c| c.kind == "service_unavailable" && c.service_id == "slope"));
        assert_eq!(r.schedule.expect("schedule").end, "2026-10-29 17:00");
    }

    #[test]
    fn flags_training_after_last_use() {
        let input = PlanInput { items: vec![item("草地", 2)], date: Some("2026-10-27".into()), start_time: Some("19:00".into()) };
        let r = run(input, &[]);
        assert!(r.conflicts.iter().any(|c| c.kind == "after_hours"));
    }

    #[test]
    fn rejects_bad_arguments() {
        assert!(validate(&PlanInput { items: vec![item("草地", 0)], date: None, start_time: None }).is_err());
        assert!(validate(&PlanInput { items: vec![item("草地", 1)], date: Some("10/29".into()), start_time: None }).is_err());
        assert!(validate(&PlanInput { items: vec![item("草地", 1)], date: None, start_time: Some("14:00".into()) }).is_err());
    }
}
