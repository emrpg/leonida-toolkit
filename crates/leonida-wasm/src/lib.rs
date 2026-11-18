mod natives;
mod rage;
mod sol;
mod v04;

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

/// Convert Leonida world `(x, y)` metres to Atlas SVG `(x, z)` coordinates.
#[wasm_bindgen]
pub fn world_to_svg(x: f64, y: f64) -> Vec<f64> {
    v04::world_to_svg(x, y).to_vec()
}

/// Convert Atlas SVG `(sx, sz)` coordinates back to Leonida world `(x, y)` metres.
#[wasm_bindgen]
pub fn svg_to_world(sx: f64, sz: f64) -> Vec<f64> {
    v04::svg_to_world(sx, sz).to_vec()
}

/// Solve 3D camera pose `(x, y, z, yaw_deg, pitch_deg, fov_deg, rmse_px)` from 3+ observations.
#[wasm_bindgen]
pub fn solve_camera_pose(
    obs: JsValue,
    width: f64,
    height: f64,
    fov_deg: Option<f64>,
) -> Result<JsValue, JsValue> {
    let parsed: Vec<v04::CameraObs> = from_js(obs)?;
    match v04::solve_camera_pose(&parsed, width, height, fov_deg) {
        Some(pose) => to_js(&pose),
        None => Ok(JsValue::NULL),
    }
}

/// Build a 2D camera frustum sector polygon (`Array<[x, y]>`) for map rendering.
#[wasm_bindgen]
pub fn frustum_polygon(
    x: f64,
    y: f64,
    yaw_deg: f64,
    fov_deg: f64,
    range_m: f64,
) -> Result<JsValue, JsValue> {
    let poly = v04::frustum_polygon(x, y, yaw_deg, fov_deg, range_m);
    to_js(&poly)
}

/// Plan a shortest-path route from `(x1, y1)` to `(x2, y2)` over a KNN graph of `nodes`.
#[wasm_bindgen]
pub fn plan_route(
    nodes: JsValue,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    k: Option<usize>,
) -> Result<JsValue, JsValue> {
    let parsed: Vec<WorldPoint> = if nodes.is_null() || nodes.is_undefined() {
        Vec::new()
    } else {
        from_js(nodes)?
    };
    let route = v04::plan_route(&parsed, x1, y1, x2, y2, k.unwrap_or(6));
    to_js(&route)
}

/// Compute 32-bit Jenkins one-at-a-time (JOAAT) hash for a RAGE asset name (case-insensitive).
#[wasm_bindgen]
pub fn joaat(name: &str) -> u32 {
    rage::joaat(name)
}

/// Compute deterministic NG key table index from asset name and file size.
#[wasm_bindgen]
pub fn compute_ng_key_index(name: &str, file_size: u64) -> u32 {
    rage::compute_ng_key_index(name, file_size)
}

/// Parse an RPF7/RPF8 header from a hex string.
#[wasm_bindgen]
pub fn parse_rpf_header(hex: &str) -> Result<JsValue, JsValue> {
    let bytes = rage::parse_hex_bytes(hex).map_err(|e| JsValue::from_str(&e))?;
    let hdr = rage::parse_rpf_header(&bytes).map_err(|e| JsValue::from_str(&e))?;
    to_js(&hdr)
}

/// Parse an RPF7/RPF8 header from raw bytes.
#[wasm_bindgen]
pub fn parse_rpf_bytes(bytes: &[u8]) -> Result<JsValue, JsValue> {
    let hdr = rage::parse_rpf_header(bytes).map_err(|e| JsValue::from_str(&e))?;
    to_js(&hdr)
}

/// Parse an RSC7 header from a hex string.
#[wasm_bindgen]
pub fn parse_rsc7_header(hex: &str) -> Result<JsValue, JsValue> {
    let bytes = rage::parse_hex_bytes(hex).map_err(|e| JsValue::from_str(&e))?;
    let hdr = rage::parse_rsc7_header(&bytes).map_err(|e| JsValue::from_str(&e))?;
    to_js(&hdr)
}

/// Parse an RSC7 header from raw bytes.
#[wasm_bindgen]
pub fn parse_rsc7_bytes(bytes: &[u8]) -> Result<JsValue, JsValue> {
    let hdr = rage::parse_rsc7_header(bytes).map_err(|e| JsValue::from_str(&e))?;
    to_js(&hdr)
}

/// Compute RSC7 segment size (bytes) from page flags.
#[wasm_bindgen]
pub fn compute_segment_size(flags: u32) -> u64 {
    rage::compute_segment_size(flags)
}

/// Parse `handling.meta` XML into `HandlingItem[]`.
#[wasm_bindgen]
pub fn parse_handling_meta(xml: &str) -> Result<JsValue, JsValue> {
    let items = rage::parse_handling_meta(xml).map_err(|e| JsValue::from_str(&e))?;
    to_js(&items)
}

/// Export `HandlingItem[]` back to `handling.meta` XML.
#[wasm_bindgen]
pub fn export_handling_meta(items: JsValue) -> Result<String, JsValue> {
    let parsed: Vec<rage::HandlingItem> = from_js(items)?;
    Ok(rage::export_handling_meta(&parsed))
}

/// Generate a ScriptHookV C++ / C# SHVDN / Rust FFI stub for a `NativeEntry`.
#[wasm_bindgen]
pub fn generate_shv_stub(entry: JsValue, lang: &str) -> Result<String, JsValue> {
    let parsed: natives::NativeEntry = from_js(entry)?;
    natives::generate_shv_stub(&parsed, lang).map_err(|e| JsValue::from_str(&e))
}

/// Filter `NativeEntry[]` by query string and/or namespace.
#[wasm_bindgen]
pub fn filter_natives(
    entries: JsValue,
    q: Option<String>,
    namespace: Option<String>,
    limit: Option<usize>,
) -> Result<JsValue, JsValue> {
    let parsed: Vec<natives::NativeEntry> = from_js(entries)?;
    let matches = natives::filter_natives(
        &parsed,
        q.as_deref(),
        namespace.as_deref(),
        limit.unwrap_or(100),
    );
    to_js(&matches)
}

/// Compute per-namespace counts for `NativeEntry[]`.
#[wasm_bindgen]
pub fn native_stats(entries: JsValue) -> Result<JsValue, JsValue> {
    let parsed: Vec<natives::NativeEntry> = from_js(entries)?;
    let stats = natives::native_stats(&parsed);
    to_js(&stats)
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

    #[test]
    fn v04_atlas_frustum_route_and_camera_pose() {
        let svg = v04::world_to_svg(100.0, -50.0);
        assert_eq!(svg, [200.0, 100.0]);
        assert_eq!(v04::svg_to_world(svg[0], svg[1]), [100.0, -50.0]);

        let poly = v04::frustum_polygon(0.0, 0.0, 0.0, 60.0, 500.0);
        assert_eq!(poly.first(), Some(&[0.0, 0.0]));
        assert_eq!(poly.last(), Some(&[0.0, 0.0]));
        assert!(poly.len() >= 18);

        let route = v04::plan_route(
            &[
                WorldPoint { x: 500.0, y: 0.0 },
                WorldPoint {
                    x: 1000.0,
                    y: 500.0,
                },
            ],
            0.0,
            0.0,
            1000.0,
            1000.0,
            4,
        );
        assert!(route.waypoints.len() >= 2);
        assert!(route.distance_m >= 1000.0 * std::f64::consts::SQRT_2 - 1e-6);
        assert!(route.drive_min > route.fly_min);

        let obs = [
            v04::CameraObs {
                world_x: -100.0,
                world_y: 500.0,
                world_z: 10.0,
                px: 418.3,
                py: 360.0,
            },
            v04::CameraObs {
                world_x: 0.0,
                world_y: 500.0,
                world_z: 50.0,
                px: 640.0,
                py: 271.32,
            },
            v04::CameraObs {
                world_x: 120.0,
                world_y: 600.0,
                world_z: 20.0,
                px: 861.7,
                py: 341.52,
            },
        ];
        let pose = v04::solve_camera_pose(&obs, 1280.0, 720.0, Some(60.0)).unwrap();
        assert!(pose.rmse_px < 10.0);
    }

    #[test]
    fn rage_rpf_rsc7_joaat_and_handling() {
        assert_eq!(rage::joaat("adder"), 0xB779_A091);

        let rpf_bytes = [
            0x52, 0x50, 0x46, 0x37, // RPF7
            0x10, 0x00, 0x00, 0x00, // entry_count = 16
            0x00, 0x02, 0x00, 0x00, // names_length = 512
            0x4F, 0x50, 0x45, 0x4E, // OPEN
        ];
        let rpf = rage::parse_rpf_header(&rpf_bytes).unwrap();
        assert_eq!(rpf.version, "RPF7");
        assert_eq!(rpf.entry_count, 16);
        assert_eq!(rpf.names_length, 512);
        assert_eq!(rpf.encryption, "Open");

        let rsc_bytes = [
            0x52, 0x53, 0x43, 0x37, // RSC7
            0x0D, 0x00, 0x00, 0x00, // version = 13
            0x00, 0x20, 0x00, 0x00, // system_flags
            0x00, 0x40, 0x00, 0x00, // graphics_flags
        ];
        let rsc = rage::parse_rsc7_header(&rsc_bytes).unwrap();
        assert_eq!(rsc.version, 13);
        assert!(rsc.system_size > 0);
        assert!(rsc.graphics_size > 0);

        let xml = r#"<CHandlingDataMgr><HandlingData><Item type="CHandlingData"><handlingName>ADDER</handlingName><fMass value="1650.0" /><nInitialDriveGears value="7" /></Item></HandlingData></CHandlingDataMgr>"#;
        let items = rage::parse_handling_meta(xml).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].handling_name, "ADDER");
        assert_eq!(items[0].n_initial_drive_gears, 7);
        let exported = rage::export_handling_meta(&items);
        let roundtrip = rage::parse_handling_meta(&exported).unwrap();
        assert_eq!(roundtrip[0].handling_name, "ADDER");
    }

    #[test]
    fn natives_stub_generation_and_filter() {
        let entry = natives::NativeEntry {
            namespace: "PLAYER".into(),
            name: "CAN_PED_HEAR_PLAYER".into(),
            hash: "0xF297383AA91DCA29".into(),
            jhash: Some("0x1C70B2EB".into()),
            params: vec![
                natives::NativeParam {
                    name: "player".into(),
                    param_type: "Player".into(),
                },
                natives::NativeParam {
                    name: "ped".into(),
                    param_type: "Ped".into(),
                },
            ],
            return_type: "BOOL".into(),
            description: "Checks if a ped can hear the player.".into(),
        };
        let cpp = natives::generate_shv_stub(&entry, "cpp").unwrap();
        assert!(cpp.contains("invoke<BOOL>(0xF297383AA91DCA29, player, ped)"));
        let cs = natives::generate_shv_stub(&entry, "csharp").unwrap();
        assert!(cs.contains("Function.Call<bool>((Hash)0xF297383AA91DCA29, player, ped)"));
        let rs = natives::generate_shv_stub(&entry, "rust").unwrap();
        assert!(rs.contains("shv_invoke::<bool>(0xF297383AA91DCA29"));
    }
}
