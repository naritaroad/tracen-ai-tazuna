//! Forgiving service names: trainers type "草地", "坂路" or "pool", not the exact record name.

use super::ServiceRow;

/// Lower case, no spaces or punctuation. Keeps CJK characters as they are.
pub fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// How well `query` names `row`: 0 = no match. Exact id or name beats an alias,
/// and a longer matching alias beats a shorter one.
fn score(query: &str, row: &ServiceRow) -> usize {
    let q = normalize(query);
    if q.is_empty() {
        return 0;
    }
    if q == normalize(&row.id) {
        return 10_000;
    }
    let names = [&row.name, &row.name_ja, &row.name_en];
    if names.iter().any(|n| normalize(n) == q) {
        return 9_000;
    }
    let mut best = 0;
    for key in names.into_iter().chain(row.aliases.iter()) {
        let k = normalize(key);
        if k.is_empty() {
            continue;
        }
        if k == q {
            best = best.max(8_000);
        } else if k.contains(&q) && q.chars().count() >= 2 {
            // "草地" inside "草地基本訓練"
            best = best.max(1_000 + q.chars().count());
        } else if q.contains(&k) && k.chars().count() >= 2 {
            // "坂路トレーニングは使えますか" contains the alias "坂路"
            best = best.max(100 + k.chars().count());
        }
    }
    best
}

/// Matching rows with their scores, best first.
pub fn rank(query: &str, rows: Vec<ServiceRow>) -> Vec<(ServiceRow, usize)> {
    let mut scored: Vec<_> = rows
        .into_iter()
        .map(|r| {
            let s = score(query, &r);
            (r, s)
        })
        .filter(|(_, s)| *s > 0)
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored
}

pub enum Resolved {
    One(ServiceRow),
    Ambiguous(Vec<ServiceRow>),
    None,
}

/// Exactly one service for a name, or the candidates when it is not clear.
pub fn resolve(query: &str, rows: &[ServiceRow]) -> Resolved {
    let ranked = rank(query, rows.to_vec());
    let Some(top) = ranked.first().map(|(_, s)| *s) else {
        return Resolved::None;
    };
    let best: Vec<ServiceRow> = ranked.into_iter().take_while(|(_, s)| *s == top).map(|(r, _)| r).collect();
    if best.len() == 1 {
        Resolved::One(best.into_iter().next().expect("one row"))
    } else {
        Resolved::Ambiguous(best)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn service(id: &str, name: &str, name_ja: &str, name_en: &str, aliases: &[&str]) -> ServiceRow {
        ServiceRow {
            id: id.into(),
            name: name.into(),
            name_ja: name_ja.into(),
            name_en: name_en.into(),
            category: "track".into(),
            unit_minutes: 60,
            capacity: 8,
            fee_jpy: 0,
            available: true,
            location: String::new(),
            aliases: aliases.iter().map(|a| a.to_string()).collect(),
            notes: String::new(),
            source_type: "project".into(),
        }
    }

    fn rows() -> Vec<ServiceRow> {
        vec![
            service("turf_basic", "草地基本訓練", "芝コース基礎トレーニング", "Turf course basic training", &["草地", "芝", "turf"]),
            service("turf_pace", "草地併跑練習", "芝コース併走トレーニング", "Turf course pacing run", &["併跑", "pacing"]),
            service("slope", "坡道訓練", "坂路トレーニング", "Uphill slope training", &["坡道", "坂路", "slope"]),
        ]
    }

    #[test]
    fn resolves_forgiving_names() {
        let rows = rows();
        assert!(matches!(resolve("turf_basic", &rows), Resolved::One(r) if r.id == "turf_basic"));
        assert!(matches!(resolve("草地", &rows), Resolved::One(r) if r.id == "turf_basic"));
        assert!(matches!(resolve("坂路トレーニングは使えますか", &rows), Resolved::One(r) if r.id == "slope"));
        assert!(matches!(resolve("Uphill Slope Training", &rows), Resolved::One(r) if r.id == "slope"));
        assert!(matches!(resolve("swimming", &rows), Resolved::None));
    }

    #[test]
    fn reports_ambiguity() {
        let mut twins = rows();
        twins.push(service("turf_basic_b", "草地基本訓練B", "", "", &["芝"]));
        assert!(matches!(resolve("芝", &twins), Resolved::Ambiguous(c) if c.len() == 2));
    }
}
