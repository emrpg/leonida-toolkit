use std::collections::HashMap;

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::db::{CreateMarkerInput, NewLeaderboardEntry, UpdateMarkerInput, MARKERS_SCHEMA};
use crate::error::AppError;
use crate::state::AppState;

const MEDIA_TABS: &[(&str, &str)] = &[
    ("trailer1", "Trailer 1"),
    ("trailer2", "Trailer 2"),
    ("trailer3", "Trailer 3"),
    ("leaks", "Leaks"),
    ("screenshots", "Screenshots"),
    ("el_screenshots", "Extra Screenshots"),
];

pub async fn auth_api(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    body: Bytes,
) -> Result<Json<Value>, AppError> {
    let action = params
        .get("action")
        .map(String::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::BadRequest("missing 'action' query parameter".into()))?;

    match action {
        "access_config" => Ok(Json(json!({
            "ok": true,
            "communityAccessEnabled": std::env::var("LEONIDA_ACCESS_CODE").is_ok_and(|v| !v.is_empty()),
            "requirePasscode": std::env::var("LEONIDA_ACCESS_CODE").is_ok_and(|v| !v.is_empty()),
            "guestAllowed": true,
            "mode": "open",
        }))),

        "access_verify" => {
            #[derive(Deserialize, Default)]
            struct VerifyBody {
                passcode: Option<String>,
            }
            let payload: VerifyBody = if body.is_empty() {
                VerifyBody::default()
            } else {
                serde_json::from_slice(&body)
                    .map_err(|e| AppError::BadRequest(format!("invalid JSON body: {e}")))?
            };
            let provided = payload
                .passcode
                .or_else(|| params.get("passcode").cloned())
                .unwrap_or_default();

            if let Ok(expected) = std::env::var("LEONIDA_ACCESS_CODE") {
                if !expected.is_empty() && provided != expected {
                    return Err(AppError::BadRequest("invalid access passcode".into()));
                }
            }
            Ok(Json(json!({
                "ok": true,
                "verified": true,
                "role": "explorer",
                "token": "leonida-guest-session",
            })))
        }

        "gtadb_source_public" => Ok(Json(json!({
            "ok": true,
            "source": "local",
            "datasets": [
                "markers",
                "trailer1",
                "trailer2",
                "trailer3",
                "leaks",
                "screenshots",
                "el_screenshots"
            ],
        }))),

        "site_settings_public" => Ok(Json(json!({
            "ok": true,
            "siteName": "State of Leonida",
            "version": env!("CARGO_PKG_VERSION"),
            "defaultLang": "zh-CN",
            "features": {
                "screenshotChallenge": true,
                "communityMarkers": true,
                "mediaGallery": true,
            }
        }))),

        "ranks" => Ok(Json(json!({
            "ok": true,
            "ranks": [
                { "id": "scout", "name": "Scout", "minScore": 0 },
                { "id": "cartographer", "name": "Cartographer", "minScore": 2500 },
                { "id": "surveyor", "name": "Master Surveyor", "minScore": 4200 },
                { "id": "legend", "name": "Leonida Legend", "minScore": 4800 }
            ]
        }))),

        "notifications" => {
            let notifications = state.markers.list_notifications()?;
            let unread_count = notifications.iter().filter(|n| !n.read).count();
            Ok(Json(json!({
                "ok": true,
                "unreadCount": unread_count,
                "notifications": notifications,
            })))
        }

        "notifications_read" => {
            #[derive(Deserialize, Default)]
            struct ReadBody {
                id: Option<String>,
            }
            let payload: ReadBody = if body.is_empty() {
                ReadBody::default()
            } else {
                serde_json::from_slice(&body)
                    .map_err(|e| AppError::BadRequest(format!("invalid JSON body: {e}")))?
            };
            let target_id = payload.id.or_else(|| params.get("id").cloned());
            let updated = state
                .markers
                .mark_notifications_read(target_id.as_deref())?;
            Ok(Json(json!({
                "ok": true,
                "updated": updated,
            })))
        }

        "marker_list" => {
            let markers = state.markers.list()?;
            Ok(Json(json!({
                "ok": true,
                "schema": MARKERS_SCHEMA,
                "markers": markers,
            })))
        }

        "marker_save" => {
            let input: CreateMarkerInput = serde_json::from_slice(&body)
                .map_err(|e| AppError::BadRequest(format!("invalid marker payload: {e}")))?;
            if input.name.trim().is_empty() {
                return Err(AppError::BadRequest("marker name must not be empty".into()));
            }
            if !input.x.is_finite() || !input.y.is_finite() {
                return Err(AppError::BadRequest(
                    "x and y must be finite numbers".into(),
                ));
            }
            let marker = state.markers.create(input)?;
            Ok(Json(json!({
                "ok": true,
                "marker": marker,
            })))
        }

        "marker_update" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct SolMarkerUpdate {
                id: String,
                name: Option<String>,
                x: Option<f64>,
                y: Option<f64>,
                note: Option<Option<String>>,
            }
            let input: SolMarkerUpdate = serde_json::from_slice(&body)
                .map_err(|e| AppError::BadRequest(format!("invalid marker payload: {e}")))?;
            if let Some(ref name) = input.name {
                if name.trim().is_empty() {
                    return Err(AppError::BadRequest("marker name must not be empty".into()));
                }
            }
            if input.x.is_some_and(|x| !x.is_finite()) || input.y.is_some_and(|y| !y.is_finite()) {
                return Err(AppError::BadRequest(
                    "x and y must be finite numbers".into(),
                ));
            }
            let updated = state
                .markers
                .update(
                    &input.id,
                    UpdateMarkerInput {
                        name: input.name,
                        x: input.x,
                        y: input.y,
                        note: input.note,
                    },
                )?
                .ok_or_else(|| AppError::NotFound(format!("marker '{}' not found", input.id)))?;
            Ok(Json(json!({
                "ok": true,
                "marker": updated,
            })))
        }

        "marker_delete" => {
            #[derive(Deserialize, Default)]
            struct DeleteBody {
                id: Option<String>,
            }
            let payload: DeleteBody = if body.is_empty() {
                DeleteBody::default()
            } else {
                serde_json::from_slice(&body)
                    .map_err(|e| AppError::BadRequest(format!("invalid JSON body: {e}")))?
            };
            let id = payload
                .id
                .or_else(|| params.get("id").cloned())
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| AppError::BadRequest("missing marker 'id'".into()))?;
            if !state.markers.delete(&id)? {
                return Err(AppError::NotFound(format!("marker '{id}' not found")));
            }
            Ok(Json(json!({
                "ok": true,
                "deleted": true,
            })))
        }

        "game_stats" => {
            let game = params
                .get("game")
                .map(String::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or("screenshot_challenge");
            let stats = state.markers.game_stats(game)?;
            Ok(Json(json!({
                "ok": true,
                "game": stats.game,
                "totalPlays": stats.total_plays,
                "bestScore": stats.best_score,
                "avgScore": stats.avg_score,
                "avgDistanceM": stats.avg_distance_m,
            })))
        }

        "leaderboard" => {
            let game = params
                .get("game")
                .map(String::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or("screenshot_challenge");
            let limit = params
                .get("limit")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(10)
                .clamp(1, 100);
            let entries = state.markers.leaderboard(game, limit)?;
            Ok(Json(json!({
                "ok": true,
                "game": game,
                "entries": entries,
            })))
        }

        other => Err(AppError::BadRequest(format!("unknown action: {other}"))),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeSubmitRequest {
    pub game: Option<String>,
    pub player: String,
    pub entry_id: Option<String>,
    pub target_x: f64,
    pub target_y: f64,
    pub guess_x: f64,
    pub guess_y: f64,
}

pub async fn submit_challenge(
    State(state): State<AppState>,
    Json(req): Json<ChallengeSubmitRequest>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let player = req.player.trim().to_string();
    if player.is_empty() {
        return Err(AppError::BadRequest("player name must not be empty".into()));
    }
    if ![req.target_x, req.target_y, req.guess_x, req.guess_y]
        .into_iter()
        .all(f64::is_finite)
    {
        return Err(AppError::BadRequest(
            "coordinates must be finite numbers".into(),
        ));
    }

    let distance_m = (req.guess_x - req.target_x).hypot(req.guess_y - req.target_y);
    let score = (5000.0 * (-distance_m / 1200.0).exp()).round() as i64;
    let grade = match score {
        4500..=5000 => "S",
        3500..=4499 => "A",
        2200..=3499 => "B",
        1000..=2199 => "C",
        _ => "D",
    }
    .to_string();

    let game = req
        .game
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "screenshot_challenge".to_string());

    let entry = state.markers.record_challenge_score(NewLeaderboardEntry {
        game,
        player,
        entry_id: req.entry_id,
        score,
        distance_m,
        grade: grade.clone(),
        guess_x: req.guess_x,
        guess_y: req.guess_y,
        target_x: req.target_x,
        target_y: req.target_y,
    })?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "ok": true,
            "entry": entry,
            "result": {
                "distanceM": distance_m,
                "score": score,
                "grade": grade,
            }
        })),
    ))
}

#[derive(Debug, Deserialize)]
pub struct MediaQuery {
    pub action: Option<String>,
    #[serde(rename = "type")]
    pub media_type: Option<String>,
}

pub async fn get_media_files(
    State(state): State<AppState>,
    Query(query): Query<MediaQuery>,
) -> Result<Json<Value>, AppError> {
    if query.action.as_deref() == Some("tabs") {
        let tabs: Vec<Value> = MEDIA_TABS
            .iter()
            .map(|(id, label)| json!({ "id": id, "label": label }))
            .collect();
        return Ok(Json(json!({
            "ok": true,
            "tabs": tabs,
        })));
    }

    let media_type = query
        .media_type
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::BadRequest("missing 'action=tabs' or 'type' parameter".into()))?;

    if !MEDIA_TABS.iter().any(|(id, _)| *id == media_type) {
        return Err(AppError::BadRequest(format!(
            "unsupported media type '{media_type}'"
        )));
    }

    let path = state.data_dir.join(format!("{media_type}.json"));
    let files: Value = match std::fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|_| json!([])),
        Err(_) => json!([]),
    };

    Ok(Json(json!({
        "ok": true,
        "type": media_type,
        "files": files,
    })))
}
