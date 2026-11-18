mod model;

use model::{
    build_irl_to_ig_model, filter_landmarks, landmark_stats, nearest_landmarks,
    parse_gta6map_tuple, path_length as core_path_length, polygon_area as core_polygon_area,
    predict_ig as core_predict_ig, to_geojson, triangulate as core_triangulate, Anchor,
    Gta6MapTuple, IrlStatus, Landmark, LandmarkFilter, LatLon, Ray2D, TileIndex, TileManifest,
    WorldPoint,
};
use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::prelude::*;

fn to_js<T: Serialize + ?Sized>(val: &T) -> Result<JsValue, JsValue> {
    val.serialize(&Serializer::json_compatible())
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

fn from_js<T: serde::de::DeserializeOwned>(val: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(val).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Handle to a loaded collection of Leonida landmarks.
#[wasm_bindgen]
pub struct Landmarks {
    list: Vec<Landmark>,
}

#[wasm_bindgen]
impl Landmarks {
    /// Create an empty `Landmarks` collection.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { list: Vec::new() }
    }

    /// Parse a `gta6map-landmarks.json` tuple array string into a `Landmarks` handle.
    pub fn load(raw_json: &str) -> Result<Landmarks, JsValue> {
        let rows: Vec<Gta6MapTuple> =
            serde_json::from_str(raw_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let list = rows
            .into_iter()
            .map(|r| parse_gta6map_tuple(r, "gta6map.github.io"))
            .collect();
        Ok(Self { list })
    }

    /// Parse an already-normalised `Landmark[]` JSON string.
    pub fn load_normalised(landmarks_json: &str) -> Result<Landmarks, JsValue> {
        let list: Vec<Landmark> =
            serde_json::from_str(landmarks_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self { list })
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// Return all landmarks as a JS array.
    pub fn all(&self) -> Result<JsValue, JsValue> {
        to_js(&self.list)
    }

    /// Filter landmarks by `{ text?, tags?, statuses?, districts?, positionedOnly? }`.
    pub fn filter(&self, filter: JsValue) -> Result<JsValue, JsValue> {
        let parsed: LandmarkFilter = if filter.is_null() || filter.is_undefined() {
            LandmarkFilter::default()
        } else {
            from_js(filter)?
        };
        let matches = filter_landmarks(&self.list, &parsed);
        to_js(&matches)
    }

    /// Return the `n` nearest positioned landmarks to `(x, y)` (default `n = 5`).
    pub fn nearest(&self, x: f64, y: f64, n: Option<usize>) -> Result<JsValue, JsValue> {
        let matches = nearest_landmarks(&self.list, WorldPoint { x, y }, n.unwrap_or(5));
        to_js(&matches)
    }

    /// Compute aggregate landmark statistics.
    pub fn stats(&self) -> Result<JsValue, JsValue> {
        let stats = landmark_stats(&self.list);
        to_js(&stats)
    }

    /// Export positioned landmarks as a GeoJSON FeatureCollection.
    pub fn geojson(&self) -> Result<JsValue, JsValue> {
        let gj = to_geojson(&self.list);
        to_js(&gj)
    }

    /// Predict in-game coordinates from `(lat, lon)` using confirmed anchors in this dataset.
    pub fn predict_ig(&self, lat: f64, lon: f64, k: Option<usize>) -> Result<JsValue, JsValue> {
        let anchors: Vec<Anchor> = self
            .list
            .iter()
            .filter(|l| l.irl_status == IrlStatus::Confirmed)
            .filter_map(|l| {
                Some(Anchor {
                    irl: l.irl?,
                    ig: l.ig?,
                })
            })
            .collect();
        let Some(model) = build_irl_to_ig_model(&anchors) else {
            return Ok(JsValue::NULL);
        };
        match core_predict_ig(&model, LatLon { lat, lon }, k.unwrap_or(10)) {
            Some(pred) => to_js(&pred),
            None => Ok(JsValue::NULL),
        }
    }
}

impl Default for Landmarks {
    fn default() -> Self {
        Self::new()
    }
}

/// Predict an in-game point from `anchors` (`Array<{ irl: { lat, lon }, ig: { x, y } }>`).
#[wasm_bindgen]
pub fn predict_ig(
    anchors: JsValue,
    lat: f64,
    lon: f64,
    k: Option<usize>,
) -> Result<JsValue, JsValue> {
    let parsed: Vec<Anchor> = from_js(anchors)?;
    let Some(model) = build_irl_to_ig_model(&parsed) else {
        return Ok(JsValue::NULL);
    };
    match core_predict_ig(&model, LatLon { lat, lon }, k.unwrap_or(10)) {
        Some(pred) => to_js(&pred),
        None => Ok(JsValue::NULL),
    }
}

/// Least-squares triangulation from N >= 2 bearing rays (`Array<{ origin: { x, y }, bearing, weight? }>`).
#[wasm_bindgen]
pub fn triangulate(rays: JsValue) -> Result<JsValue, JsValue> {
    let parsed: Vec<Ray2D> = from_js(rays)?;
    match core_triangulate(&parsed) {
        Some(res) => to_js(&res),
        None => Ok(JsValue::NULL),
    }
}

/// Total polyline length (metres) for `Array<{ x, y }>`.
#[wasm_bindgen]
pub fn path_length(points: JsValue) -> Result<f64, JsValue> {
    let parsed: Vec<WorldPoint> = from_js(points)?;
    Ok(core_path_length(&parsed))
}

/// Polygon area (m^2) via the shoelace formula for `Array<{ x, y }>`.
#[wasm_bindgen]
pub fn polygon_area(points: JsValue) -> Result<f64, JsValue> {
    let parsed: Vec<WorldPoint> = from_js(points)?;
    Ok(core_polygon_area(&parsed))
}

/// Resolves the tileset directory holding a tile, preferring the composite overlay over the base set.
#[wasm_bindgen]
pub struct TileResolver {
    index: TileIndex,
}

#[wasm_bindgen]
impl TileResolver {
    #[wasm_bindgen(constructor)]
    pub fn new(manifest: JsValue) -> Result<TileResolver, JsValue> {
        let parsed: TileManifest = from_js(manifest)?;
        Ok(Self {
            index: TileIndex::from_manifest(parsed),
        })
    }

    pub fn has(&self, set: &str, z: u32, x: u32, y: u32) -> bool {
        self.index.has(set, z, x, y)
    }

    pub fn tilesets(&self) -> Vec<String> {
        self.index.tilesets()
    }

    pub fn resolve(
        &self,
        base: &str,
        composite: Option<String>,
        z: u32,
        x: u32,
        y: u32,
    ) -> Option<String> {
        self.index.resolve(base, composite.as_deref(), z, x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::model::*;

    #[test]
    fn path_and_area_match_ts() {
        let pts = [
            WorldPoint { x: 0.0, y: 0.0 },
            WorldPoint { x: 10.0, y: 0.0 },
            WorldPoint { x: 10.0, y: 5.0 },
            WorldPoint { x: 0.0, y: 5.0 },
        ];
        assert!((path_length(&pts) - 25.0).abs() < 1e-9);
        assert!((polygon_area(&pts) - 50.0).abs() < 1e-9);
    }

    #[test]
    fn tile_resolver_prefers_composite_then_base() {
        let mut manifest = TileManifest::new();
        manifest.insert(
            "base".to_string(),
            [("0".to_string(), vec!["0,0".to_string(), "0,1".to_string()])]
                .into_iter()
                .collect(),
        );
        manifest.insert(
            "comp".to_string(),
            [("0".to_string(), vec!["0,1".to_string()])]
                .into_iter()
                .collect(),
        );
        let idx = TileIndex::from_manifest(manifest);
        assert_eq!(
            idx.resolve("base", Some("comp"), 0, 1, 0).as_deref(),
            Some("comp")
        );
        assert_eq!(
            idx.resolve("base", Some("comp"), 0, 0, 0).as_deref(),
            Some("base")
        );
        assert_eq!(idx.resolve("base", Some("comp"), 0, 2, 0), None);
    }
}
