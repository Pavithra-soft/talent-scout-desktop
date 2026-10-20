use serde::Deserialize;

use crate::db::Stored;
use crate::fields::matches_skill_query;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Filters {
    #[serde(default)]
    pub notice: String,
    pub notice_min: Option<i32>,
    pub notice_max: Option<i32>,
    #[serde(default)]
    pub locations: Vec<String>,
    #[serde(default)]
    pub skills: String,
    #[serde(default)]
    pub company: String,
    #[serde(default)]
    pub name: String,
    pub score_min: Option<i32>,
}

pub fn visible_rows(rows: Vec<Stored>, filters: &Filters) -> Vec<Stored> {
    let mut kept: Vec<Stored> = rows.into_iter().filter(|row| row.error.is_empty()).collect();
    let notice = filters.notice.to_lowercase();
    if notice == "immediate" {
        kept.retain(|row| {
            row.notice_days.is_none()
                || row.notice_days == Some(0)
                || row.notice_raw.to_lowercase().contains("serving notice")
        });
    } else if matches!(notice.as_str(), "15" | "30" | "60" | "90") {
        let limit: i32 = notice.parse().unwrap_or(0);
        kept.retain(|row| match row.notice_days {
            Some(days) => days <= limit,
            None => true,
        });
    } else if notice == "custom" {
        kept.retain(|row| row.notice_days.is_none() || in_range(row.notice_days, filters.notice_min, filters.notice_max));
    }
    if !filters.locations.is_empty() {
        kept.retain(|row| {
            filters
                .locations
                .iter()
                .any(|city| city.eq_ignore_ascii_case(&row.location))
        });
    }
    kept.retain(|row| matches_skill_query(&row.skills, &filters.skills));
    kept.retain(|row| contains(&row.company, &filters.company));
    kept.retain(|row| contains(&row.name, &filters.name));
    if let Some(min_score) = filters.score_min {
        kept.retain(|row| row.score.map(|score| score >= min_score).unwrap_or(true));
    }
    kept.sort_by(|left, right| {
        right
            .score
            .unwrap_or(-1)
            .cmp(&left.score.unwrap_or(-1))
            .then_with(|| left.name.trim().is_empty().cmp(&right.name.trim().is_empty()))
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    kept
}

fn contains(value: &str, query: &str) -> bool {
    let needle = query.trim().to_lowercase();
    needle.is_empty() || value.to_lowercase().contains(&needle)
}

fn in_range(days: Option<i32>, min_days: Option<i32>, max_days: Option<i32>) -> bool {
    let Some(days) = days else {
        return false;
    };
    if min_days.is_some_and(|min| days < min) {
        return false;
    }
    if max_days.is_some_and(|max| days > max) {
        return false;
    }
    true
}
