use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::header::CONTENT_DISPOSITION;
use axum::http::{HeaderMap, HeaderValue};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::core_adapter::LandmarkFilter;
use crate::error::AppError;
use crate::state::AppState;

pub const COMPANION_SCHEMA: &str = "leonida-companion-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectibleItem {
    pub id: String,
    pub title: String,
    pub category: String,
    pub district: String,
    pub x: f64,
    pub y: f64,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionStats {
    pub total: usize,
    pub completed: usize,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistResponse {
    pub items: Vec<CollectibleItem>,
    pub stats: CompanionStats,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleRequest {
    pub id: String,
    pub completed: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleResponse {
    pub id: String,
    pub completed: bool,
    pub stats: CompanionStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionExportBundle {
    pub schema: String,
    pub exported_at: i64,
    pub completed_ids: Vec<String>,
    pub progress: HashMap<String, bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionImportItem {
    pub id: String,
    pub completed: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionImportRequest {
    pub schema: Option<String>,
    pub completed_ids: Option<Vec<String>>,
    pub progress: Option<HashMap<String, bool>>,
    pub items: Option<Vec<CompanionImportItem>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionImportResponse {
    pub schema: String,
    pub imported: usize,
    pub stats: CompanionStats,
}

fn load_collectible_catalog(state: &AppState) -> Vec<CollectibleItem> {
    let path = state.data_dir.join("collectibles.json");
    if let Ok(raw) = std::fs::read_to_string(&path) {
        if let Ok(items) = serde_json::from_str::<Vec<CollectibleItem>>(&raw) {
            if !items.is_empty() {
                return items;
            }
        }
    }

    let mut out = vec![
        CollectibleItem {
            id: "col-vb-01".into(),
            title: "Vice Beach Lifeguard Tower".into(),
            category: "Photo Op".into(),
            district: "Vice Beach".into(),
            x: 1340.0,
            y: 2830.0,
            completed: false,
        },
        CollectibleItem {
            id: "col-dt-01".into(),
            title: "Downtown Skybridge Jump".into(),
            category: "Stunt Jump".into(),
            district: "Downtown".into(),
            x: 1000.0,
            y: 2000.0,
            completed: false,
        },
        CollectibleItem {
            id: "col-pk-01".into(),
            title: "Port Vice City Dock Cache".into(),
            category: "Hidden Package".into(),
            district: "Port Vice City".into(),
            x: 820.0,
            y: 1120.0,
            completed: false,
        },
        CollectibleItem {
            id: "col-gh-01".into(),
            title: "Grassrivers Airboat Postcard".into(),
            category: "Letter Scrap".into(),
            district: "Grassrivers".into(),
            x: -1800.0,
            y: 650.0,
            completed: false,
        },
        CollectibleItem {
            id: "col-ky-01".into(),
            title: "Leonida Keys Lighthouse".into(),
            category: "Landmark".into(),
            district: "Keys".into(),
            x: -2400.0,
            y: -4800.0,
            completed: false,
        },
    ];

    let landmarks = state.engine.filter(&LandmarkFilter {
        positioned_only: true,
        ..Default::default()
    });
    for l in landmarks.into_iter().take(15) {
        if let Some(ig) = l.ig {
            out.push(CollectibleItem {
                id: format!("lm-{}", l.id),
                title: l.name,
                category: l
                    .tags
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Landmark".to_string()),
                district: l.district.unwrap_or_else(|| "Leonida".to_string()),
                x: ig.x,
                y: ig.y,
                completed: false,
            });
        }
    }

    out
}

fn compute_stats(items: &[CollectibleItem]) -> CompanionStats {
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

fn build_checklist(state: &AppState) -> Result<(Vec<CollectibleItem>, CompanionStats), AppError> {
    let progress = state.markers.get_companion_progress()?;
    let mut items = load_collectible_catalog(state);
    for item in &mut items {
        if let Some(&done) = progress.get(&item.id) {
            item.completed = done;
        }
    }
    let stats = compute_stats(&items);
    Ok((items, stats))
}

pub async fn get_checklist(
    State(state): State<AppState>,
) -> Result<Json<ChecklistResponse>, AppError> {
    let (items, stats) = build_checklist(&state)?;
    Ok(Json(ChecklistResponse { items, stats }))
}

pub async fn toggle_item(
    State(state): State<AppState>,
    Json(req): Json<ToggleRequest>,
) -> Result<Json<ToggleResponse>, AppError> {
    let id = req.id.trim().to_string();
    if id.is_empty() {
        return Err(AppError::BadRequest(
            "collectible id must not be empty".into(),
        ));
    }
    let completed = state.markers.set_companion_item(&id, req.completed)?;
    let (_, stats) = build_checklist(&state)?;
    Ok(Json(ToggleResponse {
        id,
        completed,
        stats,
    }))
}

pub async fn export_progress(
    State(state): State<AppState>,
) -> Result<(HeaderMap, Json<CompanionExportBundle>), AppError> {
    let progress = state.markers.get_companion_progress()?;
    let mut completed_ids: Vec<String> = progress
        .iter()
        .filter(|&(_, &done)| done)
        .map(|(id, _)| id.clone())
        .collect();
    completed_ids.sort();

    let exported_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"leonida-companion-progress.json\""),
    );

    Ok((
        headers,
        Json(CompanionExportBundle {
            schema: COMPANION_SCHEMA.to_string(),
            exported_at,
            completed_ids,
            progress,
        }),
    ))
}

pub async fn import_progress(
    State(state): State<AppState>,
    Json(req): Json<CompanionImportRequest>,
) -> Result<Json<CompanionImportResponse>, AppError> {
    if let Some(ref schema) = req.schema {
        if schema != COMPANION_SCHEMA {
            return Err(AppError::BadRequest(format!(
                "unsupported companion schema '{schema}', expected '{COMPANION_SCHEMA}'"
            )));
        }
    }

    let mut entries: Vec<(String, bool)> = Vec::new();
    if let Some(progress) = req.progress {
        for (id, done) in progress {
            if !id.trim().is_empty() {
                entries.push((id, done));
            }
        }
    }
    if let Some(completed_ids) = req.completed_ids {
        for id in completed_ids {
            if !id.trim().is_empty() {
                entries.push((id, true));
            }
        }
    }
    if let Some(items) = req.items {
        for item in items {
            if !item.id.trim().is_empty() {
                entries.push((item.id, item.completed));
            }
        }
    }

    let imported = state.markers.import_companion_progress(&entries)?;
    let (_, stats) = build_checklist(&state)?;
    Ok(Json(CompanionImportResponse {
        schema: COMPANION_SCHEMA.to_string(),
        imported,
        stats,
    }))
}
