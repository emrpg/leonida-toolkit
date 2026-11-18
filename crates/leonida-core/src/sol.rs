//! State of Leonida (v0.3) domain models, dataset parsing/stats/filtering, and Screenshot Challenge scoring.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// Unified entry across State of Leonida datasets (`markers`, `trailer1..3`, `leaks`, `screenshots`, `el_screenshots`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolEntry {
    pub id: String,
    pub title: String,
    pub category: String,
    pub dataset: String,
    pub x: f64,
    pub y: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub district: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp_sec: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_world: Option<String>,
}

/// Summary statistics for a collection of [`SolEntry`] items.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolDatasetStats {
    pub total: usize,
    pub by_dataset: BTreeMap<String, usize>,
    pub by_category: BTreeMap<String, usize>,
    pub by_district: BTreeMap<String, usize>,
    pub by_status: BTreeMap<String, usize>,
}

/// Filter criteria for [`filter_sol_entries`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SolEntryFilter {
    pub text: Option<String>,
    pub dataset: Option<String>,
    pub category: Option<String>,
    pub district: Option<String>,
    pub status: Option<String>,
}

/// Grade tier for Screenshot Challenge guesses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChallengeGrade {
    S,
    A,
    B,
    C,
    D,
}

/// Result of scoring a Screenshot Challenge guess against the target coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeScoreResult {
    pub distance_m: f64,
    pub score: u32,
    pub grade: ChallengeGrade,
}

/// Public site configuration returned by `site_settings_public`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SiteSettings {
    pub site_title: String,
    pub maintenance_mode: bool,
    pub community_access_enabled: bool,
    pub default_language: String,
    pub default_tileset: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motd: Option<String>,
}

/// User rank definition returned by `ranks`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RankInfo {
    pub id: String,
    pub name: String,
    pub level: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub badge_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Notification item returned by `notifications`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationItem {
    pub id: String,
    pub title: String,
    pub message: String,
    pub created_at: i64,
    #[serde(default)]
    pub read: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// News article item from `news.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub published_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Database changelog entry from `database_changelog.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogEntry {
    pub version: String,
    pub date: String,
    pub summary: String,
    #[serde(default)]
    pub changes: Vec<String>,
}

/// Score a Screenshot Challenge guess against the target `(tx, ty)` in world metres.
///
/// Uses exponential decay: `score = round(5000 * exp(-distance_m / 1200))` clamped to `0..=5000`.
pub fn score_screenshot_challenge(tx: f64, ty: f64, gx: f64, gy: f64) -> ChallengeScoreResult {
    let distance_m = (gx - tx).hypot(gy - ty);
    if !distance_m.is_finite() {
        return ChallengeScoreResult {
            distance_m,
            score: 0,
            grade: ChallengeGrade::D,
        };
    }

    let raw = (5000.0 * (-distance_m / 1200.0).exp()).round();
    let score = raw.clamp(0.0, 5000.0) as u32;
    let grade = match score {
        4500..=5000 => ChallengeGrade::S,
        3500..=4499 => ChallengeGrade::A,
        2000..=3499 => ChallengeGrade::B,
        1000..=1999 => ChallengeGrade::C,
        _ => ChallengeGrade::D,
    };

    ChallengeScoreResult {
        distance_m,
        score,
        grade,
    }
}

/// Parse a JSON array of [`SolEntry`] records.
pub fn parse_sol_entries(json_str: &str) -> Result<Vec<SolEntry>, CoreError> {
    Ok(serde_json::from_str(json_str)?)
}

/// Compute counts grouped by dataset, category, district, and status.
pub fn sol_dataset_stats(entries: &[SolEntry]) -> SolDatasetStats {
    let mut stats = SolDatasetStats {
        total: entries.len(),
        ..Default::default()
    };

    for e in entries {
        *stats.by_dataset.entry(e.dataset.clone()).or_insert(0) += 1;
        *stats.by_category.entry(e.category.clone()).or_insert(0) += 1;
        let district = e.district.as_deref().unwrap_or("(none)");
        *stats.by_district.entry(district.to_string()).or_insert(0) += 1;
        let status = e.status.as_deref().unwrap_or("(none)");
        *stats.by_status.entry(status.to_string()).or_insert(0) += 1;
    }

    stats
}

/// Group [`SolEntry`] items by their `dataset` field.
pub fn group_sol_entries_by_dataset(entries: &[SolEntry]) -> BTreeMap<String, Vec<SolEntry>> {
    let mut grouped: BTreeMap<String, Vec<SolEntry>> = BTreeMap::new();
    for e in entries {
        grouped
            .entry(e.dataset.clone())
            .or_default()
            .push(e.clone());
    }
    grouped
}

/// Filter [`SolEntry`] items by text query and exact dataset/category/district/status fields.
pub fn filter_sol_entries(entries: &[SolEntry], filter: &SolEntryFilter) -> Vec<SolEntry> {
    let terms: Vec<String> = filter
        .text
        .as_deref()
        .map(|t| {
            t.to_lowercase()
                .split_whitespace()
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    entries
        .iter()
        .filter(|e| {
            if let Some(ds) = filter.dataset.as_deref() {
                if e.dataset != ds {
                    return false;
                }
            }
            if let Some(cat) = filter.category.as_deref() {
                if e.category != cat {
                    return false;
                }
            }
            if let Some(dist) = filter.district.as_deref() {
                if e.district.as_deref() != Some(dist) {
                    return false;
                }
            }
            if let Some(status) = filter.status.as_deref() {
                if e.status.as_deref() != Some(status) {
                    return false;
                }
            }
            if !terms.is_empty() {
                let hay = format!(
                    "{}\n{}\n{}\n{}\n{}\n{}\n{}",
                    e.id,
                    e.title,
                    e.category,
                    e.dataset,
                    e.district.as_deref().unwrap_or(""),
                    e.description.as_deref().unwrap_or(""),
                    e.real_world.as_deref().unwrap_or(""),
                )
                .to_lowercase();
                if !terms.iter().all(|t| hay.contains(t.as_str())) {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}
