use std::collections::HashMap;

use anyhow::Result;
use serde_json::Value;

pub use leonida_core::{
    build_irl_to_ig_model, filter_landmarks, id_sort_key, landmark_stats, nearest_landmarks,
    parse_gta6map_tuple, predict_ig, to_geojson, triangulate, Anchor, Gta6MapTuple, IrlStatus,
    IrlToIgModel, Landmark, LandmarkFilter, LandmarkStats, LatLon, NearestLandmark, Prediction,
    Ray2D, Triangulation, WorldPoint,
};

pub fn parse_irl_status(s: &str) -> Option<IrlStatus> {
    match s.trim().to_ascii_lowercase().as_str() {
        "confirmed" => Some(IrlStatus::Confirmed),
        "unconfirmed" => Some(IrlStatus::Unconfirmed),
        "unknown" => Some(IrlStatus::Unknown),
        _ => None,
    }
}

/// Core computation boundary backed by `leonida_core`.
pub trait LandmarkEngine: Send + Sync {
    fn find_by_id(&self, id: &str) -> Option<&Landmark>;
    fn filter(&self, filter: &LandmarkFilter) -> Vec<Landmark>;
    fn nearest(&self, p: WorldPoint, n: usize) -> Vec<NearestLandmark>;
    fn stats(&self) -> &LandmarkStats;
    fn geojson(&self, filter: &LandmarkFilter) -> Value;
    fn predict_ig(&self, irl: LatLon, k: usize) -> Option<Prediction>;
    fn triangulate(&self, rays: &[Ray2D]) -> Option<Triangulation>;
}

#[derive(Debug, Clone)]
pub struct CoreEngine {
    landmarks: Vec<Landmark>,
    index_by_id: HashMap<String, usize>,
    stats: LandmarkStats,
    irl_model: Option<IrlToIgModel>,
}

impl CoreEngine {
    pub fn from_gta6map_json(raw: &str) -> Result<Self> {
        let rows: Vec<Gta6MapTuple> = serde_json::from_str(raw)?;
        let mut landmarks: Vec<Landmark> = rows
            .iter()
            .map(|r| parse_gta6map_tuple(r, "gta6map.github.io"))
            .collect();
        landmarks.sort_by_key(|a| id_sort_key(&a.id));
        Ok(Self::from_landmarks(landmarks))
    }

    pub fn from_landmarks(landmarks: Vec<Landmark>) -> Self {
        let mut index_by_id = HashMap::with_capacity(landmarks.len());
        for (i, l) in landmarks.iter().enumerate() {
            index_by_id.insert(l.id.clone(), i);
        }
        let stats = landmark_stats(&landmarks);
        let anchors: Vec<Anchor> = landmarks
            .iter()
            .filter_map(|l| {
                if l.irl_status == IrlStatus::Confirmed {
                    Some(Anchor {
                        irl: l.irl?,
                        ig: l.ig?,
                    })
                } else {
                    None
                }
            })
            .collect();
        let irl_model = build_irl_to_ig_model(&anchors).ok();
        Self {
            landmarks,
            index_by_id,
            stats,
            irl_model,
        }
    }
}

impl LandmarkEngine for CoreEngine {
    fn find_by_id(&self, id: &str) -> Option<&Landmark> {
        self.index_by_id.get(id).map(|&i| &self.landmarks[i])
    }

    fn filter(&self, filter: &LandmarkFilter) -> Vec<Landmark> {
        filter_landmarks(&self.landmarks, filter)
    }

    fn nearest(&self, p: WorldPoint, n: usize) -> Vec<NearestLandmark> {
        nearest_landmarks(&self.landmarks, p, n)
    }

    fn stats(&self) -> &LandmarkStats {
        &self.stats
    }

    fn geojson(&self, filter: &LandmarkFilter) -> Value {
        let filtered = filter_landmarks(&self.landmarks, filter);
        serde_json::to_value(to_geojson(&filtered)).expect("GeoJsonFeatureCollection serializes")
    }

    fn predict_ig(&self, irl: LatLon, k: usize) -> Option<Prediction> {
        let model = self.irl_model.as_ref()?;
        predict_ig(model, irl, k)
    }

    fn triangulate(&self, rays: &[Ray2D]) -> Option<Triangulation> {
        triangulate(rays)
    }
}
