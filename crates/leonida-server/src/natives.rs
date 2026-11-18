use std::collections::BTreeMap;

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::db::{NativeBookmark, SaveNativeBookmarkInput};
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeParam {
    pub name: String,
    #[serde(rename = "paramType", alias = "type", alias = "param_type")]
    pub param_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeEntry {
    pub namespace: String,
    pub name: String,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jhash: Option<String>,
    #[serde(default)]
    pub params: Vec<NativeParam>,
    #[serde(alias = "return_type", default)]
    pub return_type: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NativeNamespaceStat {
    pub namespace: String,
    pub count: usize,
}

#[derive(Debug, Deserialize)]
pub struct NativesQuery {
    pub q: Option<String>,
    pub ns: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct NativesListResponse {
    pub total: usize,
    pub items: Vec<NativeEntry>,
}

pub async fn list_natives(
    State(state): State<AppState>,
    Query(query): Query<NativesQuery>,
) -> Json<NativesListResponse> {
    let ns_filter = query
        .ns
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase);
    let terms: Vec<String> = query
        .q
        .as_deref()
        .map(|s| {
            s.split_whitespace()
                .map(|t| t.to_ascii_lowercase())
                .collect()
        })
        .unwrap_or_default();
    let limit = query.limit.unwrap_or(100).clamp(1, 500);

    let mut matched: Vec<NativeEntry> = state
        .natives
        .iter()
        .filter(|entry| {
            if let Some(ref ns) = ns_filter {
                if entry.namespace.to_ascii_lowercase() != *ns {
                    return false;
                }
            }
            if !terms.is_empty() {
                let hay = format!(
                    "{} {} {} {} {}",
                    entry.namespace,
                    entry.name,
                    entry.hash,
                    entry.jhash.as_deref().unwrap_or(""),
                    entry.description
                )
                .to_ascii_lowercase();
                if !terms.iter().all(|t| hay.contains(t.as_str())) {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect();

    let total = matched.len();
    matched.truncate(limit);
    Json(NativesListResponse {
        total,
        items: matched,
    })
}

#[derive(Debug, Serialize)]
pub struct NativesStatsResponse {
    pub total: usize,
    pub namespaces: Vec<NativeNamespaceStat>,
}

pub async fn natives_stats(State(state): State<AppState>) -> Json<NativesStatsResponse> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for entry in state.natives.iter() {
        *counts.entry(entry.namespace.clone()).or_insert(0) += 1;
    }
    let mut namespaces: Vec<NativeNamespaceStat> = counts
        .into_iter()
        .map(|(namespace, count)| NativeNamespaceStat { namespace, count })
        .collect();
    namespaces.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.namespace.cmp(&b.namespace))
    });

    Json(NativesStatsResponse {
        total: state.natives.len(),
        namespaces,
    })
}

#[derive(Debug, Serialize)]
pub struct BookmarksResponse {
    pub bookmarks: Vec<NativeBookmark>,
}

pub async fn list_bookmarks(
    State(state): State<AppState>,
) -> Result<Json<BookmarksResponse>, AppError> {
    let bookmarks = state.markers.list_native_bookmarks()?;
    Ok(Json(BookmarksResponse { bookmarks }))
}

#[derive(Debug, Serialize)]
pub struct BookmarkResponse {
    pub bookmark: NativeBookmark,
}

pub async fn save_bookmark(
    State(state): State<AppState>,
    Json(input): Json<SaveNativeBookmarkInput>,
) -> Result<(StatusCode, Json<BookmarkResponse>), AppError> {
    if input.hash.trim().is_empty() {
        return Err(AppError::BadRequest("native hash must not be empty".into()));
    }
    let bookmark = state.markers.save_native_bookmark(input)?;
    Ok((StatusCode::CREATED, Json(BookmarkResponse { bookmark })))
}

#[derive(Debug, Deserialize)]
pub struct DeleteBookmarkQuery {
    pub hash: Option<String>,
    pub id: Option<String>,
}

pub async fn delete_bookmark_query_or_body(
    State(state): State<AppState>,
    Query(query): Query<DeleteBookmarkQuery>,
    body: Bytes,
) -> Result<Json<Value>, AppError> {
    #[derive(Deserialize, Default)]
    struct DeleteBody {
        hash: Option<String>,
        id: Option<String>,
    }
    let parsed: DeleteBody = if body.is_empty() {
        DeleteBody::default()
    } else {
        serde_json::from_slice(&body)
            .map_err(|e| AppError::BadRequest(format!("invalid JSON body: {e}")))?
    };
    let hash = query
        .hash
        .or(query.id)
        .or(parsed.hash)
        .or(parsed.id)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("missing 'hash'".into()))?;

    if !state.markers.delete_native_bookmark(&hash)? {
        return Err(AppError::NotFound(format!("bookmark '{hash}' not found")));
    }
    Ok(Json(json!({ "deleted": true })))
}

pub async fn delete_bookmark_path(
    State(state): State<AppState>,
    Path(hash): Path<String>,
) -> Result<Json<Value>, AppError> {
    if !state.markers.delete_native_bookmark(&hash)? {
        return Err(AppError::NotFound(format!("bookmark '{hash}' not found")));
    }
    Ok(Json(json!({ "deleted": true })))
}
