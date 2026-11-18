use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_DISPOSITION};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use crate::companion::{export_progress, get_checklist, import_progress, toggle_item};
use crate::core_adapter::{
    parse_irl_status, CameraObs, CameraPose, Landmark, LandmarkFilter, LandmarkStats, LatLon,
    NearestLandmark, Prediction, Ray2D, RouteResult, Triangulation, WorldPoint,
};
use crate::db::{
    CreateMarkerInput, ImportMarkersInput, MarkerBundle, PhotogrammetryProject, SaveProjectInput,
    UpdateMarkerInput, UserMarker, MARKERS_SCHEMA,
};
use crate::error::AppError;
use crate::phone::{
    create_message, delete_note_path, delete_note_query_or_body, delete_photo_path,
    delete_photo_query_or_body, list_contacts, list_messages, list_notes, list_photos, save_note,
    save_photo,
};
use crate::rage::{export_handling, inspect_header, parse_handling};
use crate::sol::{auth_api, get_media_files, submit_challenge};
use crate::state::AppState;

pub fn build_router(state: AppState) -> Router {
    let tiles_service = axum::routing::get_service(ServeDir::new(&state.tiles_dir)).layer(
        SetResponseHeaderLayer::overriding(
            CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=86400"),
        ),
    );
    let data_service = ServeDir::new(&state.data_dir);
    let phone_service = ServeDir::new(&state.phone_dist_dir).append_index_html_on_directories(true);

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
        .route("/api/photogrammetry/solve", post(solve_photogrammetry))
        .route(
            "/api/photogrammetry/projects",
            get(list_projects)
                .post(save_project)
                .delete(delete_project_query_or_body),
        )
        .route(
            "/api/photogrammetry/projects/{id}",
            delete(delete_project_path),
        )
        .route("/api/route", get(plan_route))
        .route("/api/companion/checklist", get(get_checklist))
        .route("/api/companion/toggle", post(toggle_item))
        .route("/api/companion/export", get(export_progress))
        .route("/api/companion/import", post(import_progress))
        .route("/api/rage/inspect-header", post(inspect_header))
        .route("/api/rage/handling/parse", post(parse_handling))
        .route("/api/rage/handling/export", post(export_handling))
        .route("/api/phone/contacts", get(list_contacts))
        .route(
            "/api/phone/messages",
            get(list_messages).post(create_message),
        )
        .route(
            "/api/phone/notes",
            get(list_notes)
                .post(save_note)
                .delete(delete_note_query_or_body),
        )
        .route("/api/phone/notes/{id}", delete(delete_note_path))
        .route(
            "/api/phone/photos",
            get(list_photos)
                .post(save_photo)
                .delete(delete_photo_query_or_body),
        )
        .route("/api/phone/photos/{id}", delete(delete_photo_path))
        .route("/auth/api.php", get(auth_api).post(auth_api))
        .route("/api/sol/challenge/submit", post(submit_challenge))
        .route("/get-media-files.php", get(get_media_files))
        .nest_service("/tiles", tiles_service)
        .nest_service("/data", data_service)
        .nest_service("/phone", phone_service)
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
            let parsed = parse_irl_status(&s)
                .ok_or_else(|| AppError::BadRequest(format!("invalid status: {s}")))?;
            statuses.push(parsed);
        }
        Ok(LandmarkFilter {
            text: self.q.filter(|s| !s.trim().is_empty()),
            tags: (!tags.is_empty()).then_some(tags),
            statuses: (!statuses.is_empty()).then_some(statuses),
            districts: (!districts.is_empty()).then_some(districts),
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
    Ok(Json(state.engine.filter(&filter)))
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PhotogrammetrySolveRequest {
    observations: Vec<CameraObs>,
    width: f64,
    height: f64,
    #[serde(alias = "fov_deg")]
    fov_deg: Option<f64>,
    #[serde(alias = "range_m")]
    range_m: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhotogrammetrySolveResponse {
    pose: CameraPose,
    frustum: Vec<[f64; 2]>,
}

async fn solve_photogrammetry(
    State(state): State<AppState>,
    Json(req): Json<PhotogrammetrySolveRequest>,
) -> Result<Json<PhotogrammetrySolveResponse>, AppError> {
    if req.observations.len() < 3 {
        return Err(AppError::BadRequest(
            "photogrammetry solve requires at least 3 observations".into(),
        ));
    }
    if !req.width.is_finite() || !req.height.is_finite() || req.width <= 0.0 || req.height <= 0.0 {
        return Err(AppError::BadRequest(
            "width and height must be positive finite numbers".into(),
        ));
    }
    for obs in &req.observations {
        if ![obs.world_x, obs.world_y, obs.world_z, obs.px, obs.py]
            .into_iter()
            .all(f64::is_finite)
        {
            return Err(AppError::BadRequest(
                "observation coordinates must be finite numbers".into(),
            ));
        }
    }

    let fov_deg = req.fov_deg.unwrap_or(60.0);
    let range_m = req.range_m.unwrap_or(1500.0).clamp(50.0, 20000.0);
    let pose = state
        .engine
        .solve_camera_pose(&req.observations, req.width, req.height, fov_deg)
        .ok_or_else(|| AppError::Unprocessable("unable to solve camera pose".into()))?;
    let frustum = state
        .engine
        .frustum_polygon(pose.x, pose.y, pose.yaw_deg, pose.fov_deg, range_m);

    Ok(Json(PhotogrammetrySolveResponse { pose, frustum }))
}

#[derive(Debug, Serialize)]
struct ProjectListResponse {
    projects: Vec<PhotogrammetryProject>,
}

async fn list_projects(
    State(state): State<AppState>,
) -> Result<Json<ProjectListResponse>, AppError> {
    let projects = state.markers.list_projects()?;
    Ok(Json(ProjectListResponse { projects }))
}

#[derive(Debug, Serialize)]
struct SaveProjectResponse {
    project: PhotogrammetryProject,
}

async fn save_project(
    State(state): State<AppState>,
    Json(input): Json<SaveProjectInput>,
) -> Result<(StatusCode, Json<SaveProjectResponse>), AppError> {
    if input.name.trim().is_empty() {
        return Err(AppError::BadRequest(
            "project name must not be empty".into(),
        ));
    }
    if !input.width.is_finite()
        || !input.height.is_finite()
        || input.width <= 0.0
        || input.height <= 0.0
    {
        return Err(AppError::BadRequest(
            "width and height must be positive finite numbers".into(),
        ));
    }
    let project = state.markers.save_project(input)?;
    Ok((StatusCode::CREATED, Json(SaveProjectResponse { project })))
}

#[derive(Debug, Deserialize)]
struct DeleteProjectQuery {
    id: Option<String>,
}

async fn delete_project_query_or_body(
    State(state): State<AppState>,
    Query(query): Query<DeleteProjectQuery>,
    body: Bytes,
) -> Result<Json<DeleteResult>, AppError> {
    #[derive(Deserialize, Default)]
    struct DeleteBody {
        id: Option<String>,
    }
    let body_id = if body.is_empty() {
        None
    } else {
        serde_json::from_slice::<DeleteBody>(&body)
            .map_err(|e| AppError::BadRequest(format!("invalid JSON body: {e}")))?
            .id
    };
    let id = query
        .id
        .or(body_id)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("missing project 'id'".into()))?;

    if !state.markers.delete_project(&id)? {
        return Err(AppError::NotFound(format!("project '{id}' not found")));
    }
    Ok(Json(DeleteResult { deleted: true }))
}

async fn delete_project_path(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResult>, AppError> {
    if !state.markers.delete_project(&id)? {
        return Err(AppError::NotFound(format!("project '{id}' not found")));
    }
    Ok(Json(DeleteResult { deleted: true }))
}

#[derive(Debug, Deserialize)]
struct RouteQuery {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

async fn plan_route(
    State(state): State<AppState>,
    Query(q): Query<RouteQuery>,
) -> Result<Json<RouteResult>, AppError> {
    if ![q.x1, q.y1, q.x2, q.y2].into_iter().all(f64::is_finite) {
        return Err(AppError::BadRequest(
            "x1, y1, x2, y2 must be finite numbers".into(),
        ));
    }
    Ok(Json(state.engine.plan_route(
        WorldPoint { x: q.x1, y: q.y1 },
        WorldPoint { x: q.x2, y: q.y2 },
    )))
}
