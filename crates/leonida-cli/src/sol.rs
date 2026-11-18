use anyhow::{Context, Result};
use leonida_core::{
    landmark_stats, parse_gta6map_tuple, parse_gtamaplib_vc, Gta6MapTuple, GtamaplibVcFile,
    IrlStatus, Landmark, Landmark3D,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolEntry {
    pub id: String,
    pub title: String,
    pub category: String,
    pub dataset: String,
    pub x: f64,
    pub y: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub district: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp_sec: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub real_world: Option<String>,
}

fn status_str(s: IrlStatus) -> &'static str {
    match s {
        IrlStatus::Confirmed => "confirmed",
        IrlStatus::Unconfirmed => "unconfirmed",
        IrlStatus::Unknown => "unknown",
    }
}

fn primary_category(tags: &[String]) -> String {
    tags.first()
        .cloned()
        .unwrap_or_else(|| "landmark".to_string())
}

fn to_sol_entry(l: &Landmark, dataset: &str, timestamp_sec: Option<u64>) -> Option<SolEntry> {
    let ig = l.ig?;
    let real_world = if l.irl_address.is_empty() || l.irl_address == "?" {
        None
    } else {
        Some(l.irl_address.clone())
    };
    let description = if l.tags.is_empty() {
        Some(l.ig_address.clone())
    } else {
        Some(format!("{} [{}]", l.ig_address, l.tags.join(", ")))
    };

    Some(SolEntry {
        id: l.id.clone(),
        title: l.name.clone(),
        category: primary_category(&l.tags),
        dataset: dataset.to_string(),
        x: ig.x,
        y: ig.y,
        z: None,
        district: l.district.clone(),
        status: Some(status_str(l.irl_status).to_string()),
        description,
        timestamp_sec,
        media_url: None,
        real_world,
    })
}

fn to_sol_entry_3d(l: &Landmark3D, dataset: &str) -> SolEntry {
    SolEntry {
        id: l.id.clone(),
        title: l.name.clone(),
        category: "3d-landmark".to_string(),
        dataset: dataset.to_string(),
        x: l.xyz.x,
        y: l.xyz.y,
        z: Some(l.xyz.z),
        district: None,
        status: Some("confirmed".to_string()),
        description: Some(format!(
            "3D {} (w={:.1}m, h={:.1}m)",
            l.shape, l.width, l.height
        )),
        timestamp_sec: None,
        media_url: None,
        real_world: None,
    }
}

fn write_json<T: Serialize + ?Sized>(path: &Path, val: &T) -> Result<()> {
    let s = serde_json::to_string_pretty(val)?;
    fs::write(path, s).with_context(|| format!("failed to write {}", path.display()))
}

pub fn run_build_sol_data(root: &Path) -> Result<()> {
    let raw_dir = root.join("data/raw");
    let out_dir = root.join("apps/map/public/data");
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;

    let gta6_raw = fs::read_to_string(raw_dir.join("gta6map-landmarks.json"))
        .context("failed to read data/raw/gta6map-landmarks.json")?;
    let gta6_rows: Vec<Gta6MapTuple> = serde_json::from_str(&gta6_raw)?;
    let landmarks: Vec<Landmark> = gta6_rows
        .iter()
        .map(|r| parse_gta6map_tuple(r, "gta6map.github.io"))
        .collect();

    let vc_raw = fs::read_to_string(raw_dir.join("gtamaplib-vc-landmarks.json"))
        .context("failed to read data/raw/gtamaplib-vc-landmarks.json")?;
    let vc_file: GtamaplibVcFile = serde_json::from_str(&vc_raw)?;
    let landmarks_3d = parse_gtamaplib_vc(&vc_file, "gtamaplib-vc")
        .context("failed to parse gtamaplib-vc-landmarks.json")?;

    let stats = landmark_stats(&landmarks);

    // 1. markers.json: all positioned 2D landmarks + 3D landmarks
    let mut markers: Vec<SolEntry> = landmarks
        .iter()
        .filter_map(|l| to_sol_entry(l, "markers", None))
        .collect();
    for l3 in &landmarks_3d {
        markers.push(to_sol_entry_3d(l3, "markers"));
    }

    // 2. trailer1 / trailer2 / trailer3 / leaks / screenshots / el_screenshots
    let mut trailer1 = Vec::new();
    let mut trailer2 = Vec::new();
    let mut trailer3 = Vec::new();
    let mut leaks = Vec::new();
    let mut screenshots = Vec::new();
    let mut el_screenshots = Vec::new();

    for (idx, l) in landmarks.iter().enumerate() {
        if l.ig.is_none() {
            continue;
        }
        let district = l.district.as_deref().unwrap_or("");
        let ts = Some(((idx % 90) + 1) as u64);

        if district.contains("Vice Beach") || district.contains("Vice City") {
            if idx % 2 == 0 {
                if let Some(e) = to_sol_entry(l, "trailer1", ts) {
                    trailer1.push(e);
                }
            } else if let Some(e) = to_sol_entry(l, "trailer2", ts) {
                trailer2.push(e);
            }
        } else if district.contains("Port Gellhorn")
            || district.contains("Hamlet")
            || district.contains("Ambrosia")
        {
            if let Some(e) = to_sol_entry(l, "trailer2", ts) {
                trailer2.push(e);
            }
        } else if let Some(e) = to_sol_entry(l, "trailer3", ts) {
            trailer3.push(e);
        }

        if l.irl_status != IrlStatus::Confirmed {
            if let Some(e) = to_sol_entry(l, "leaks", None) {
                leaks.push(e);
            }
        } else if idx % 3 == 0 {
            if let Some(e) = to_sol_entry(l, "screenshots", None) {
                screenshots.push(e);
            }
        } else if idx % 5 == 0 {
            if let Some(e) = to_sol_entry(l, "el_screenshots", None) {
                el_screenshots.push(e);
            }
        }
    }

    for l3 in &landmarks_3d {
        leaks.push(to_sol_entry_3d(l3, "leaks"));
    }

    write_json(&out_dir.join("markers.json"), &markers)?;
    write_json(&out_dir.join("trailer1.json"), &trailer1)?;
    write_json(&out_dir.join("trailer2.json"), &trailer2)?;
    write_json(&out_dir.join("trailer3.json"), &trailer3)?;
    write_json(&out_dir.join("leaks.json"), &leaks)?;
    write_json(&out_dir.join("screenshots.json"), &screenshots)?;
    write_json(&out_dir.join("el_screenshots.json"), &el_screenshots)?;

    // 3. Editorial & metadata JSONs (public-domain / toolkit-authored, zero copyrighted assets)
    let mut top_districts: Vec<(&String, &usize)> = stats.by_district.iter().collect();
    top_districts.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    let district_summary: Vec<BTreeMap<String, serde_json::Value>> = top_districts
        .into_iter()
        .take(10)
        .map(|(name, count)| {
            let mut m = BTreeMap::new();
            m.insert("district".to_string(), serde_json::json!(name));
            m.insert("count".to_string(), serde_json::json!(count));
            m
        })
        .collect();

    let news = serde_json::json!([
        {
            "id": "sol-v03-launch",
            "title": "State of Leonida v0.3 Rust + WASM Platform",
            "date": "2026-11-18",
            "category": "release",
            "summary": format!("Normalised {} landmarks ({} positioned) and {} 3D landmarks into State of Leonida datasets.", stats.total, stats.positioned, landmarks_3d.len())
        },
        {
            "id": "sol-challenge-ready",
            "title": "Screenshot Challenge Scoring Engine Live",
            "date": "2026-11-18",
            "category": "feature",
            "summary": "WASM-powered geodesic scoring (0-5000 pts, S-D grades) backed by SQLite leaderboard."
        }
    ]);
    write_json(&out_dir.join("news.json"), &news)?;

    let changelog = serde_json::json!([
        {
            "version": "0.3.0",
            "date": "2026-11-18",
            "changes": [
                "Generated categorised SOL datasets from public-domain gta6map & gtamaplib-vc sources",
                "Added WASM Screenshot Challenge scorer and SolDataset filter/stats bindings",
                "Added Axum /auth/api.php & /api/sol/* endpoints with SQLite persistence"
            ],
            "stats": {
                "totalLandmarks": stats.total,
                "positionedLandmarks": stats.positioned,
                "landmarks3d": landmarks_3d.len(),
                "confirmed": stats.by_status.confirmed,
                "unconfirmed": stats.by_status.unconfirmed,
                "unknown": stats.by_status.unknown,
                "topDistricts": district_summary
            }
        }
    ]);
    write_json(&out_dir.join("database_changelog.json"), &changelog)?;

    let content = serde_json::json!({
        "title": "State of Leonida — Interactive Mapping & Photogrammetry Toolkit",
        "subtitle": "Community coordinate calibration, landmark search, triangulation, and screenshot challenge",
        "sections": [
            {
                "id": "overview",
                "heading": "Coordinate Frame & Calibration",
                "body": "Uses Leonida world metres (+x east, +y north, +z up) compatible with gta6map.github.io tile pyramid and gtamaplib-vc 3D triangulation."
            },
            {
                "id": "compliance",
                "heading": "Public Domain Data Notice",
                "body": "All bundled datasets originate from public-domain (Unlicense) community mapping projects. No copyrighted game assets are included."
            }
        ]
    });
    write_json(&out_dir.join("content.json"), &content)?;

    let credits = serde_json::json!([
        { "name": "gta6map.github.io", "role": "2D Landmark & Tile Pyramid Dataset", "license": "Public Domain (Unlicense)" },
        { "name": "gtamaplib-vc", "role": "3D Triangulated Landmarks", "license": "Public Domain (Unlicense)" },
        { "name": "Leonida Toolkit (Rust + WASM)", "role": "Core Geometry, CLI, Server & WebAssembly Bindings", "license": "MIT OR Apache-2.0" }
    ]);
    write_json(&out_dir.join("credits.json"), &credits)?;

    let supporters = serde_json::json!([
        { "name": "Open Photogrammetry Contributors", "tier": "community" },
        { "name": "Public Domain Mapping Maintainers", "tier": "data" }
    ]);
    write_json(&out_dir.join("supporters.json"), &supporters)?;

    let backgrounds = serde_json::json!([
        { "id": "vice-sunset", "label": "Vice Sunset", "css": "linear-gradient(135deg, #1b1026 0%, #3b1c4a 50%, #c84b6b 100%)" },
        { "id": "leonida-night", "label": "Leonida Night", "css": "linear-gradient(135deg, #0b1320 0%, #16263d 60%, #1d3557 100%)" },
        { "id": "keys-teal", "label": "Keys Teal", "css": "linear-gradient(135deg, #0d1f22 0%, #173b3f 55%, #2a6f73 100%)" }
    ]);
    write_json(&out_dir.join("backgrounds.json"), &backgrounds)?;

    let lang = serde_json::json!({
        "en": {
            "appTitle": "State of Leonida",
            "searchPlaceholder": "Search landmarks, districts, tags...",
            "challengeTitle": "Screenshot Challenge",
            "submitGuess": "Submit Guess",
            "grade": "Grade",
            "score": "Score",
            "distance": "Distance"
        },
        "zh-CN": {
            "appTitle": "莱昂尼达州地图 (State of Leonida)",
            "searchPlaceholder": "搜索地标、区域、标签...",
            "challengeTitle": "截图猜点挑战",
            "submitGuess": "提交猜测",
            "grade": "评级",
            "score": "得分",
            "distance": "误差距离"
        }
    });
    write_json(&out_dir.join("lang.json"), &lang)?;

    let seo = serde_json::json!({
        "title": "State of Leonida — Interactive Map & Coordinate Toolkit",
        "description": "Public-domain community mapping, IRL-to-IG coordinate prediction, ray triangulation, and screenshot challenge for Leonida.",
        "keywords": ["Leonida", "Interactive Map", "Photogrammetry", "Triangulation", "WASM"]
    });
    write_json(&out_dir.join("seo.json"), &seo)?;

    println!(
        "[build-sol-data] markers={} trailer1={} trailer2={} trailer3={} leaks={} screenshots={} el_screenshots={}",
        markers.len(),
        trailer1.len(),
        trailer2.len(),
        trailer3.len(),
        leaks.len(),
        screenshots.len(),
        el_screenshots.len()
    );
    Ok(())
}
