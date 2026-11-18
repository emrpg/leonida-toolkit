use axum::extract::{Path, Query, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_DISPOSITION};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use crate::core_adapter::{
    IrlStatus, Landmark, LandmarkFilter, LandmarkStats, LatLon, NearestLandmark, Prediction, Ray2D,
    Triangulation, WorldPoint,
};
use crate::db::{
    CreateMarkerInput, ImportMarkersInput, MarkerBundle, UpdateMarkerInput, UserMarker,
    MARKERS_SCHEMA,
};
use crate::error::AppError;
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    let tiles_service = axum::routing::get_service(ServeDir::new(&state.tiles_dir)).layer(
        SetResponseHeaderLayer::overriding(
            CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=86400"),
        ),
    );

    let dist_service = ServeDir::new(&state.dist_dir).append_index_html_on_directories(true);

    Router::new()
        .route("/api/health", get(health))
        .route("/api/landmarks", get(list_landmarks))
        .route("/api/landmarks/{id}", get(get_landmark))
        .route("/api/nearest", get(nearest_landmarks))
        .route("/api/predict", get(predict_ig))
        .route("/api/triangulate", post(triangulate))
        .route("/api/stats", get(stats))
        .route("/api/tile-manifest", get(tile_manifest))
        .route("/api/geojson", get(geojson))
        .route(
            "/api/markers",
            get(list_markers).post(create_marker).delete(clear_markers),
        )
        .route("/api/markers/export", get(export_markers))
        .route("/api/markers/import", post(import_markers))
        .route(
            "/api/markers/{id}",
            put(update_marker).delete(delete_marker),
        )
        .nest_service("/tiles", tiles_service)
        .fallback_service(dist_service)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

#[derive(Debug, Deserialize)]
struct FilterQuery {
    q: Option<String>,
    tags: Option<String>,
    status: Option<String>,
    district: Option<String>,
    positioned: Option<bool>,
}

impl FilterQuery {
    fn into_filter(self) -> Result<LandmarkFilter, AppError> {
        let tags = split_csv(self.tags.as_deref());
        let districts = split_csv(self.district.as_deref());
        let mut statuses = Vec::new();
        for s in split_csv(self.status.as_deref()) {
            let parsed = IrlStatus::parse(&s)
                .ok_or_else(|| AppError::BadRequest(format!("invalid status: {s}")))?;
            statuses.push(parsed);
        }
        Ok(LandmarkFilter {
            text: self.q.filter(|s| !s.trim().is_empty()),
            tags,
            statuses,
            districts,
            positioned_only: self.positioned.unwrap_or(false),
        })
    }
}

fn split_csv(raw: Option<&str>) -> Vec<String> {
    raw.map(|s| {
        s.split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect()
    })
    .unwrap_or_default()
}

async fn list_landmarks(
    State(state): State<AppState>,
    Query(query): Query<FilterQuery>,
) -> Result<Json<Vec<Landmark>>, AppError> {
    let filter = query.into_filter()?;
    let items: Vec<Landmark> = state.engine.filter(&filter).into_iter().cloned().collect();
    Ok(Json(items))
}

async fn get_landmark(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Landmark>, AppError> {
    state
        .engine
        .find_by_id(&id)
        .cloned()
        .map(Json)
        .ok_or_else(|| AppError::NotFound(format!("landmark '{id}' not found")))
}

#[derive(Debug, Deserialize)]
struct NearestQuery {
    x: f64,
    y: f64,
    n: Option<usize>,
}

async fn nearest_landmarks(
    State(state): State<AppState>,
    Query(query): Query<NearestQuery>,
) -> Result<Json<Vec<NearestLandmark>>, AppError> {
    if !query.x.is_finite() || !query.y.is_finite() {
        return Err(AppError::BadRequest(
            "x and y must be finite numbers".into(),
        ));
    }
    let n = query.n.unwrap_or(5).clamp(1, 100);
    Ok(Json(state.engine.nearest(
        WorldPoint {
            x: query.x,
            y: query.y,
        },
        n,
    )))
}

#[derive(Debug, Deserialize)]
struct PredictQuery {
    lat: f64,
    lon: f64,
    k: Option<usize>,
}

async fn predict_ig(
    State(state): State<AppState>,
    Query(query): Query<PredictQuery>,
) -> Result<Json<Prediction>, AppError> {
    if !query.lat.is_finite() || !query.lon.is_finite() {
        return Err(AppError::BadRequest(
            "lat and lon must be finite numbers".into(),
        ));
    }
    let k = query.k.unwrap_or(10).clamp(3, 50);
    state
        .engine
        .predict_ig(
            LatLon {
                lat: query.lat,
                lon: query.lon,
            },
            k,
        )
        .map(Json)
        .ok_or_else(|| AppError::Unprocessable("unable to fit local affine model".into()))
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TriangulatePayload {
    Wrapped { rays: Vec<Ray2D> },
    Bare(Vec<Ray2D>),
}

async fn triangulate(
    State(state): State<AppState>,
    Json(payload): Json<TriangulatePayload>,
) -> Result<Json<Triangulation>, AppError> {
    let rays = match payload {
        TriangulatePayload::Wrapped { rays } => rays,
        TriangulatePayload::Bare(rays) => rays,
    };
    if rays.len() < 2 {
        return Err(AppError::BadRequest(
            "triangulation requires at least 2 rays".into(),
        ));
    }
    state
        .engine
        .triangulate(&rays)
        .map(Json)
        .ok_or_else(|| AppError::Unprocessable("rays are parallel or degenerate".into()))
}

async fn stats(State(state): State<AppState>) -> Json<LandmarkStats> {
    Json(state.engine.stats().clone())
}

async fn tile_manifest(State(state): State<AppState>) -> Json<Value> {
    Json((*state.tile_manifest).clone())
}

async fn geojson(
    State(state): State<AppState>,
    Query(query): Query<FilterQuery>,
) -> Result<Json<Value>, AppError> {
    let filter = query.into_filter()?;
    Ok(Json(state.engine.geojson(&filter)))
}

async fn list_markers(State(state): State<AppState>) -> Result<Json<MarkerBundle>, AppError> {
    let markers = state.markers.list()?;
    Ok(Json(MarkerBundle {
        schema: MARKERS_SCHEMA.to_string(),
        markers,
    }))
}

async fn create_marker(
    State(state): State<AppState>,
    Json(input): Json<CreateMarkerInput>,
) -> Result<(StatusCode, Json<UserMarker>), AppError> {
    if input.name.trim().is_empty() {
        return Err(AppError::BadRequest("marker name must not be empty".into()));
    }
    if !input.x.is_finite() || !input.y.is_finite() {
        return Err(AppError::BadRequest(
            "x and y must be finite numbers".into(),
        ));
    }
    let marker = state.markers.create(input)?;
    Ok((StatusCode::CREATED, Json(marker)))
}

async fn update_marker(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateMarkerInput>,
) -> Result<Json<UserMarker>, AppError> {
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
    state
        .markers
        .update(&id, input)?
        .map(Json)
        .ok_or_else(|| AppError::NotFound(format!("marker '{id}' not found")))
}

#[derive(Serialize)]
struct DeleteResult {
    deleted: bool,
}

async fn delete_marker(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResult>, AppError> {
    if !state.markers.delete(&id)? {
        return Err(AppError::NotFound(format!("marker '{id}' not found")));
    }
    Ok(Json(DeleteResult { deleted: true }))
}

#[derive(Serialize)]
struct ClearResult {
    deleted: usize,
}

async fn clear_markers(State(state): State<AppState>) -> Result<Json<ClearResult>, AppError> {
    let deleted = state.markers.clear()?;
    Ok(Json(ClearResult { deleted }))
}

async fn export_markers(
    State(state): State<AppState>,
) -> Result<(HeaderMap, Json<MarkerBundle>), AppError> {
    let markers = state.markers.list()?;
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"leonida-my-markers.json\""),
    );
    Ok((
        headers,
        Json(MarkerBundle {
            schema: MARKERS_SCHEMA.to_string(),
            markers,
        }),
    ))
}

#[derive(Serialize)]
struct ImportResult {
    schema: String,
    imported: usize,
    markers: Vec<UserMarker>,
}

async fn import_markers(
    State(state): State<AppState>,
    Json(payload): Json<ImportMarkersInput>,
) -> Result<Json<ImportResult>, AppError> {
    if let Some(ref schema) = payload.schema {
        if schema != MARKERS_SCHEMA {
            return Err(AppError::BadRequest(format!(
                "unsupported marker schema '{schema}', expected '{MARKERS_SCHEMA}'"
            )));
        }
    }
    for m in &payload.markers {
        if m.name.trim().is_empty() {
            return Err(AppError::BadRequest("marker name must not be empty".into()));
        }
        if !m.x.is_finite() || !m.y.is_finite() {
            return Err(AppError::BadRequest(
                "x and y must be finite numbers".into(),
            ));
        }
    }
    let markers = state.markers.import(payload.markers)?;
    Ok(Json(ImportResult {
        schema: MARKERS_SCHEMA.to_string(),
        imported: markers.len(),
        markers,
    }))
}
