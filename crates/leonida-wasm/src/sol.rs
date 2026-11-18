use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolEntry {
    pub id: String,
    pub title: String,
    pub category: String,
    pub dataset: String,
    pub x: f64,
    pub y: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub district: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp_sec: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub real_world: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolDatasetStats {
    pub total: usize,
    pub by_category: BTreeMap<String, usize>,
    pub by_dataset: BTreeMap<String, usize>,
    pub by_district: BTreeMap<String, usize>,
    pub by_status: BTreeMap<String, usize>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolFilter {
    pub text: Option<String>,
    pub category: Option<String>,
    pub dataset: Option<String>,
    pub district: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChallengeScoreResult {
    pub distance_m: f64,
    pub score: u32,
    pub grade: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NearestSolEntry<'a> {
    pub entry: &'a SolEntry,
    pub distance: f64,
}

pub fn score_screenshot_challenge(
    target_x: f64,
    target_y: f64,
    guess_x: f64,
    guess_y: f64,
) -> ChallengeScoreResult {
    let distance_m = (guess_x - target_x).hypot(guess_y - target_y);
    let raw = (5000.0 * (-distance_m / 1200.0).exp()).round();
    let score = raw.clamp(0.0, 5000.0) as u32;
    let grade = match score {
        4500..=5000 => "S",
        3500..=4499 => "A",
        2200..=3499 => "B",
        1000..=2199 => "C",
        _ => "D",
    }
    .to_string();
    ChallengeScoreResult {
        distance_m,
        score,
        grade,
    }
}

pub fn sol_stats(entries: &[SolEntry]) -> SolDatasetStats {
    let mut stats = SolDatasetStats {
        total: entries.len(),
        by_category: BTreeMap::new(),
        by_dataset: BTreeMap::new(),
        by_district: BTreeMap::new(),
        by_status: BTreeMap::new(),
    };
    for e in entries {
        *stats.by_category.entry(e.category.clone()).or_insert(0) += 1;
        *stats.by_dataset.entry(e.dataset.clone()).or_insert(0) += 1;
        let d = e.district.as_deref().unwrap_or("(none)").to_string();
        *stats.by_district.entry(d).or_insert(0) += 1;
        let s = e.status.as_deref().unwrap_or("unknown").to_string();
        *stats.by_status.entry(s).or_insert(0) += 1;
    }
    stats
}

pub fn filter_sol_entries<'a>(entries: &'a [SolEntry], f: &SolFilter) -> Vec<&'a SolEntry> {
    let terms: Vec<String> = f
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
            if let Some(cat) = f.category.as_deref().filter(|s| !s.is_empty()) {
                if !e.category.eq_ignore_ascii_case(cat) {
                    return false;
                }
            }
            if let Some(ds) = f.dataset.as_deref().filter(|s| !s.is_empty()) {
                if !e.dataset.eq_ignore_ascii_case(ds) {
                    return false;
                }
            }
            if let Some(dist) = f.district.as_deref().filter(|s| !s.is_empty()) {
                if e.district.as_deref() != Some(dist) {
                    return false;
                }
            }
            if let Some(st) = f.status.as_deref().filter(|s| !s.is_empty()) {
                if e.status.as_deref() != Some(st) {
                    return false;
                }
            }
            if !terms.is_empty() {
                let hay = format!(
                    "{}\n{}\n{}\n{}\n{}",
                    e.id,
                    e.title,
                    e.category,
                    e.district.as_deref().unwrap_or(""),
                    e.description.as_deref().unwrap_or("")
                )
                .to_lowercase();
                if !terms.iter().all(|t| hay.contains(t.as_str())) {
                    return false;
                }
            }
            true
        })
        .collect()
}

pub fn nearest_sol_entries<'a>(
    entries: &'a [SolEntry],
    x: f64,
    y: f64,
    n: usize,
) -> Vec<NearestSolEntry<'a>> {
    let mut ranked: Vec<NearestSolEntry<'a>> = entries
        .iter()
        .map(|entry| NearestSolEntry {
            entry,
            distance: (entry.x - x).hypot(entry.y - y),
        })
        .collect();
    ranked.sort_by(|a, b| a.distance.total_cmp(&b.distance));
    ranked.truncate(n);
    ranked
}
