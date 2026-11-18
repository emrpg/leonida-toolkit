//! Companion checklist & collectible progress tracking models (v0.5).

use serde::{Deserialize, Serialize};

/// A trackable collectible or companion checklist item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectibleItem {
    pub id: String,
    pub title: String,
    pub category: String,
    #[serde(default)]
    pub district: String,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub completed: bool,
}

/// Summary completion statistics for a companion checklist.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionStats {
    pub total: usize,
    pub completed: usize,
    pub percent: f64,
}

/// Compute completion statistics for a slice of [`CollectibleItem`] entries.
pub fn companion_stats(items: &[CollectibleItem]) -> CompanionStats {
    let total = items.len();
    let completed = items.iter().filter(|i| i.completed).count();
    let percent = if total == 0 {
        0.0
    } else {
        (completed as f64 / total as f64) * 100.0
    };
    CompanionStats {
        total,
        completed,
        percent,
    }
}

/// Filter [`CollectibleItem`] entries by optional category and completion status.
pub fn filter_collectibles(
    items: &[CollectibleItem],
    category: Option<&str>,
    completed: Option<bool>,
) -> Vec<CollectibleItem> {
    items
        .iter()
        .filter(|item| {
            if let Some(cat) = category {
                if item.category != cat {
                    return false;
                }
            }
            if let Some(done) = completed {
                if item.completed != done {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}
