mod sol;

use leonida_core::{
    build_irl_to_ig_model, filter_landmarks, landmark_stats, nearest_landmarks,
    parse_gta6map_tuple, path_length as core_path_length, polygon_area as core_polygon_area,
    predict_ig as core_predict_ig, to_geojson, triangulate as core_triangulate, Anchor,
    Gta6MapTuple, IrlStatus, Landmark, LandmarkFilter, LatLon, Ray2D, TileCoord, TileManifest,
    TileResolver as CoreTileResolver, TileSource, WorldPoint,
};
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::prelude::*;

fn to_js<T: Serialize + ?Sized>(val: &T) -> Result<JsValue, JsValue> {
    val.serialize(&Serializer::json_compatible())
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

fn from_js<T: serde::de::DeserializeOwned>(val: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(val).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsLandmarkFilter {
    text: Option<String>,
    tags: Option<Vec<String>>,
    statuses: Option<Vec<IrlStatus>>,
    districts: Option<Vec<String>>,
    #[serde(default)]
    positioned_only: bool,
}

impl From<JsLandmarkFilter> for LandmarkFilter {
    fn from(f: JsLandmarkFilter) -> Self {
        Self {
            text: f.text,
            tags: f.tags,
            statuses: f.statuses,
            districts: f.districts,
            positioned_only: f.positioned_only,
        }
    }
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
            .iter()
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
            let js_filter: JsLandmarkFilter = from_js(filter)?;
            js_filter.into()
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
        let Ok(model) = build_irl_to_ig_model(&anchors) else {
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
    let Ok(model) = build_irl_to_ig_model(&parsed) else {
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
    inner: CoreTileResolver,
}

#[wasm_bindgen]
impl TileResolver {
    #[wasm_bindgen(constructor)]
    pub fn new(manifest: JsValue) -> Result<TileResolver, JsValue> {
        let parsed: TileManifest = from_js(manifest)?;
        Ok(Self {
            inner: CoreTileResolver::new(&parsed),
        })
    }

    pub fn has(&self, set: &str, z: u32, x: u32, y: u32) -> bool {
        let Ok(z) = u8::try_from(z) else {
            return false;
        };
        self.inner.has(
            set,
            TileCoord {
                z,
                x: x as i32,
                y: y as i32,
            },
        )
    }

    pub fn tilesets(&self) -> Vec<String> {
        self.inner
            .tilesets()
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    pub fn resolve(
        &self,
        base: &str,
        composite: Option<String>,
        z: u32,
        x: u32,
        y: u32,
    ) -> Option<String> {
        let z = u8::try_from(z).ok()?;
        let src = TileSource {
            base: base.to_string(),
            composite,
        };
        self.inner
            .resolve(
                &src,
                TileCoord {
                    z,
                    x: x as i32,
                    y: y as i32,
                },
            )
            .map(str::to_string)
    }
}

/// Score a Screenshot Challenge guess against the target coordinate.
#[wasm_bindgen]
pub fn score_screenshot_challenge(
    target_x: f64,
    target_y: f64,
    guess_x: f64,
    guess_y: f64,
) -> Result<JsValue, JsValue> {
    let result = sol::score_screenshot_challenge(target_x, target_y, guess_x, guess_y);
    to_js(&result)
}

/// Handle to a loaded State of Leonida dataset (`SolEntry[]`).
#[wasm_bindgen]
pub struct SolDataset {
    entries: Vec<sol::SolEntry>,
}

#[wasm_bindgen]
impl SolDataset {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn load(raw_json: &str) -> Result<SolDataset, JsValue> {
        let entries: Vec<sol::SolEntry> =
            serde_json::from_str(raw_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self { entries })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn all(&self) -> Result<JsValue, JsValue> {
        to_js(&self.entries)
    }

    pub fn filter(&self, filter: JsValue) -> Result<JsValue, JsValue> {
        let parsed: sol::SolFilter = if filter.is_null() || filter.is_undefined() {
            sol::SolFilter::default()
        } else {
            from_js(filter)?
        };
        let matches = sol::filter_sol_entries(&self.entries, &parsed);
        to_js(&matches)
    }

    pub fn stats(&self) -> Result<JsValue, JsValue> {
        let stats = sol::sol_stats(&self.entries);
        to_js(&stats)
    }

    pub fn nearest(&self, x: f64, y: f64, n: Option<usize>) -> Result<JsValue, JsValue> {
        let matches = sol::nearest_sol_entries(&self.entries, x, y, n.unwrap_or(5));
        to_js(&matches)
    }
}

impl Default for SolDataset {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_and_area_match_ts() {
        let pts = [
            WorldPoint { x: 0.0, y: 0.0 },
            WorldPoint { x: 10.0, y: 0.0 },
            WorldPoint { x: 10.0, y: 5.0 },
            WorldPoint { x: 0.0, y: 5.0 },
        ];
        assert!((core_path_length(&pts) - 25.0).abs() < 1e-9);
        assert!((core_polygon_area(&pts) - 50.0).abs() < 1e-9);
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
        let resolver = CoreTileResolver::new(&manifest);
        let src = TileSource {
            base: "base".to_string(),
            composite: Some("comp".to_string()),
        };
        assert_eq!(
            resolver.resolve(&src, TileCoord { z: 0, x: 1, y: 0 }),
            Some("comp")
        );
        assert_eq!(
            resolver.resolve(&src, TileCoord { z: 0, x: 0, y: 0 }),
            Some("base")
        );
        assert_eq!(resolver.resolve(&src, TileCoord { z: 0, x: 2, y: 0 }), None);
    }

    #[test]
    fn screenshot_challenge_scoring() {
        let exact = sol::score_screenshot_challenge(100.0, 200.0, 100.0, 200.0);
        assert_eq!(exact.score, 5000);
        assert_eq!(exact.grade, "S");
        assert!(exact.distance_m < 1e-9);

        let mid = sol::score_screenshot_challenge(0.0, 0.0, 1200.0, 0.0);
        assert!((mid.distance_m - 1200.0).abs() < 1e-9);
        assert_eq!(mid.score, 1839);
        assert_eq!(mid.grade, "C");

        let far = sol::score_screenshot_challenge(0.0, 0.0, 5000.0, 0.0);
        assert_eq!(far.grade, "D");
    }

    #[test]
    fn sol_dataset_filter_and_stats() {
        let entries = vec![
            sol::SolEntry {
                id: "b1".into(),
                title: "Hotel".into(),
                category: "hotel".into(),
                dataset: "trailer1".into(),
                x: 100.0,
                y: 200.0,
                z: None,
                district: Some("Vice Beach".into()),
                status: Some("confirmed".into()),
                description: None,
                timestamp_sec: Some(10),
                media_url: None,
                real_world: None,
            },
            sol::SolEntry {
                id: "b2".into(),
                title: "Port".into(),
                category: "transport".into(),
                dataset: "trailer2".into(),
                x: 300.0,
                y: 400.0,
                z: None,
                district: Some("Port Gellhorn".into()),
                status: Some("unknown".into()),
                description: None,
                timestamp_sec: Some(20),
                media_url: None,
                real_world: None,
            },
        ];
        let stats = sol::sol_stats(&entries);
        assert_eq!(stats.total, 2);
        assert_eq!(stats.by_category["hotel"], 1);
        assert_eq!(stats.by_dataset["trailer2"], 1);

        let filtered = sol::filter_sol_entries(
            &entries,
            &sol::SolFilter {
                category: Some("hotel".into()),
                ..Default::default()
            },
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "b1");
    }
}
