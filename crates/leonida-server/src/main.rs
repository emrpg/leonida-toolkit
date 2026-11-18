use std::env;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use serde_json::{json, Value};
use tracing::info;
use tracing_subscriber::EnvFilter;

use leonida_server::core_adapter::CoreEngine;
use leonida_server::db::MarkerStore;
use leonida_server::natives::NativeEntry;
use leonida_server::{build_router, AppState};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("leonida_server=info,tower_http=info")),
        )
        .init();

    let data_dir = env_path("GTA6MAP_DATA", "apps/map/public/data");
    let tiles_dir = env_path("GTA6MAP_TILES", "apps/map/public/tiles");
    let dist_dir = env_path("GTA6MAP_DIST", "apps/map/dist");
    let phone_dist_dir = env_path("LEONIDA_PHONE_DIST", "apps/phone/dist");
    let natives_dist_dir = env_path("LEONIDA_NATIVES_DIST", "apps/natives/dist");
    let natives_json_path = env_path(
        "LEONIDA_NATIVES_JSON",
        "apps/natives/public/data/natives.json",
    );
    let db_path = env_path("LEONIDA_DB", "data/leonida-markers.db");
    let addr: SocketAddr = env::var("LEONIDA_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
        .parse()
        .context("parse LEONIDA_ADDR")?;

    let landmarks_path = data_dir.join("gta6map-landmarks.json");
    let engine = if landmarks_path.exists() {
        let raw = std::fs::read_to_string(&landmarks_path)
            .with_context(|| format!("read {}", landmarks_path.display()))?;
        Arc::new(CoreEngine::from_gta6map_json(&raw)?)
    } else {
        tracing::warn!(
            "landmarks file {} not found; starting with empty dataset",
            landmarks_path.display()
        );
        Arc::new(CoreEngine::from_landmarks(Vec::new()))
    };

    let tile_manifest = load_tile_manifest(&data_dir.join("tile-manifest.json"));
    let natives = load_natives(&natives_json_path);
    let markers = MarkerStore::open(&db_path)?;

    let state = AppState {
        engine,
        markers,
        tile_manifest: Arc::new(tile_manifest),
        natives: Arc::new(natives),
        dist_dir: dist_dir.clone(),
        tiles_dir: tiles_dir.clone(),
        data_dir,
        phone_dist_dir,
        natives_dist_dir,
    };

    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(
        "leonida-server listening on http://{addr} (dist={}, tiles={}, db={})",
        dist_dir.display(),
        tiles_dir.display(),
        db_path.display()
    );
    axum::serve(listener, app).await?;
    Ok(())
}

fn env_path(key: &str, default: &str) -> PathBuf {
    env::var_os(key)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}

fn load_tile_manifest(path: &Path) -> Value {
    match std::fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|err| {
            tracing::warn!("invalid tile-manifest {}: {err}", path.display());
            json!({})
        }),
        Err(_) => json!({}),
    }
}

fn load_natives(path: &Path) -> Vec<NativeEntry> {
    match std::fs::read_to_string(path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|err| {
            tracing::warn!("invalid natives.json {}: {err}", path.display());
            Vec::new()
        }),
        Err(_) => Vec::new(),
    }
}
