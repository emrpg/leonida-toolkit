use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use crate::core_adapter::LandmarkEngine;
use crate::db::MarkerStore;
use crate::natives::NativeEntry;

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<dyn LandmarkEngine>,
    pub markers: MarkerStore,
    pub tile_manifest: Arc<Value>,
    pub natives: Arc<Vec<NativeEntry>>,
    pub dist_dir: PathBuf,
    pub tiles_dir: PathBuf,
    pub data_dir: PathBuf,
    pub phone_dist_dir: PathBuf,
    pub natives_dist_dir: PathBuf,
}
