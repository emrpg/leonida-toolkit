mod phone;
mod rage;
mod sol;
mod v04;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use leonida_core::{
    build_irl_to_ig_model, filter_landmarks, landmark_stats, parse_gta6map_tuple, predict_ig,
    to_geojson, triangulate, Anchor, Gta6MapTuple, IrlStatus, Landmark, LandmarkFilter, LatLon,
    Ray2D, WorldPoint,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Parser, Debug)]
#[command(
    name = "leonida",
    version,
    about = "Leonida (GTA VI) mapping toolkit CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Normalise raw public-domain datasets into apps/map/public/data and build tile-manifest.json
    BuildData {
        /// Workspace root directory
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Directory containing tile sets (default: <root>/apps/map/public/tiles)
        #[arg(long)]
        tiles: Option<PathBuf>,
    },
    /// Extract and classify raw datasets into State of Leonida JSONs under apps/map/public/data/
    BuildSolData {
        /// Workspace root directory
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Print landmark dataset statistics
    Stats {
        /// Path to gta6map-landmarks.json
        #[arg(long)]
        data: Option<PathBuf>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Search landmarks by text, tags, or IRL status
    Search {
        /// Search query (space-separated terms)
        query: String,
        /// Filter by tags (comma-separated)
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        /// Filter by IRL status
        #[arg(long, value_enum)]
        status: Option<StatusArg>,
        /// Only include landmarks with in-game coordinates
        #[arg(long)]
        positioned: bool,
        /// Maximum results to display
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Path to gta6map-landmarks.json
        #[arg(long)]
        data: Option<PathBuf>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Predict in-game (x, y) from real-world (lat, lon) via KNN local affine fit
    Predict {
        /// Real-world latitude (WGS84)
        #[arg(allow_hyphen_values = true)]
        lat: f64,
        /// Real-world longitude (WGS84)
        #[arg(allow_hyphen_values = true)]
        lon: f64,
        /// Number of nearest anchors to use
        #[arg(short, long, default_value_t = 10)]
        k: usize,
        /// Path to gta6map-landmarks.json
        #[arg(long)]
        data: Option<PathBuf>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Triangulate an in-game point from 2+ bearing rays (x,y,bearing[,weight])
    Triangulate {
        /// Rays formatted as x,y,bearing or x,y,bearing,weight
        #[arg(required = true, num_args = 2.., allow_hyphen_values = true)]
        rays: Vec<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Export positioned landmarks as GeoJSON (urn:leonida:world-metres)
    ExportGeojson {
        /// Output file path (prints to stdout if omitted)
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Path to gta6map-landmarks.json
        #[arg(long)]
        data: Option<PathBuf>,
    },
    /// Plan a shortest-path route from (x1, y1) to (x2, y2) over the landmark KNN graph
    Route {
        /// Start X (world metres)
        #[arg(allow_hyphen_values = true)]
        x1: f64,
        /// Start Y (world metres)
        #[arg(allow_hyphen_values = true)]
        y1: f64,
        /// Goal X (world metres)
        #[arg(allow_hyphen_values = true)]
        x2: f64,
        /// Goal Y (world metres)
        #[arg(allow_hyphen_values = true)]
        y2: f64,
        /// Number of nearest neighbours per node
        #[arg(short, long, default_value_t = 6)]
        k: usize,
        /// Path to gta6map-landmarks.json
        #[arg(long)]
        data: Option<PathBuf>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Solve 3D camera pose from 3+ photogrammetry observations (wx,wy,wz,px,py)
    SolveCamera {
        /// Image width in pixels
        #[arg(long, default_value_t = 1280.0)]
        width: f64,
        /// Image height in pixels
        #[arg(long, default_value_t = 720.0)]
        height: f64,
        /// Horizontal field of view in degrees
        #[arg(long)]
        fov: Option<f64>,
        /// Observations formatted as world_x,world_y,world_z,px,py
        #[arg(required = true, num_args = 3.., allow_hyphen_values = true)]
        obs: Vec<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// RAGE asset/config tools (RPF7/RPF8, RSC7, JOAAT, handling.meta)
    Rage(rage::RageArgs),
    /// LeonidaOS companion phone app diagnostics & stats
    Phone(phone::PhoneArgs),
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum StatusArg {
    Confirmed,
    Unconfirmed,
    Unknown,
}

impl From<StatusArg> for IrlStatus {
    fn from(value: StatusArg) -> Self {
        match value {
            StatusArg::Confirmed => IrlStatus::Confirmed,
            StatusArg::Unconfirmed => IrlStatus::Unconfirmed,
            StatusArg::Unknown => IrlStatus::Unknown,
        }
    }
}

fn default_data_path() -> PathBuf {
    let public_path = PathBuf::from("apps/map/public/data/gta6map-landmarks.json");
    if public_path.exists() {
        public_path
    } else {
        PathBuf::from("data/raw/gta6map-landmarks.json")
    }
}

fn load_landmarks(path: Option<&Path>) -> Result<Vec<Landmark>> {
    let resolved = path.map(PathBuf::from).unwrap_or_else(default_data_path);
    let raw = fs::read_to_string(&resolved)
        .with_context(|| format!("failed to read landmarks from {}", resolved.display()))?;
    let rows: Vec<Gta6MapTuple> = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse {}", resolved.display()))?;
    Ok(rows
        .iter()
        .map(|row| parse_gta6map_tuple(row, "gta6map.github.io"))
        .collect())
}

fn iso8601_utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = rem / 3600;
    let min = (rem % 3600) / 60;
    let sec = rem % 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}T{hour:02}:{min:02}:{sec:02}Z")
}

fn parse_tile_filename(name: &str) -> Option<(u32, u32)> {
    let stem = name.strip_suffix(".jpg")?;
    let mut parts = stem.split(',');
    let z: u32 = parts.next()?.parse().ok()?;
    let y: u32 = parts.next()?.parse().ok()?;
    let x: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    let _ = z;
    Some((y, x))
}

fn run_build_data(root: &Path, tiles_opt: Option<&Path>) -> Result<()> {
    let tiles_dir = match tiles_opt {
        Some(p) if p.is_absolute() => p.to_path_buf(),
        Some(p) => root.join(p),
        None => root.join("apps/map/public/tiles"),
    };
    let out_dir = root.join("apps/map/public/data");
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;

    let raw_dir = root.join("data/raw");
    let sources_meta = [
        (
            "gta6map.github.io",
            "gta6map-landmarks.json",
            "Public Domain (Unlicense)",
            "https://github.com/gta6map/gta6map.github.io",
        ),
        (
            "gtamaplib-vc",
            "gtamaplib-vc-landmarks.json",
            "Public Domain (Unlicense)",
            "https://github.com/rolux/gtamaplib-vc",
        ),
    ];

    let mut source_entries = Vec::new();
    let mut summary_parts = Vec::new();

    for (id, file, license, url) in sources_meta {
        let src_path = raw_dir.join(file);
        let raw_text = fs::read_to_string(&src_path)
            .with_context(|| format!("failed to read {}", src_path.display()))?;
        let json: Value = serde_json::from_str(&raw_text)
            .with_context(|| format!("invalid JSON in {}", src_path.display()))?;
        let compact = serde_json::to_string(&json)?;
        fs::write(out_dir.join(file), compact)?;

        let records = if let Some(arr) = json.as_array() {
            arr.len()
        } else {
            json.get("landmarks")
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
        };
        summary_parts.push(format!("{id}={records}"));
        source_entries.push(serde_json::json!({
            "id": id,
            "file": file,
            "license": license,
            "url": url,
            "records": records,
        }));
    }

    let mut manifest: BTreeMap<String, BTreeMap<String, Vec<String>>> = BTreeMap::new();
    if tiles_dir.exists() {
        for set_entry in fs::read_dir(&tiles_dir)? {
            let set_entry = set_entry?;
            let set_path = set_entry.path();
            if !set_path.is_dir() {
                continue;
            }
            let set_name = set_entry.file_name().to_string_lossy().into_owned();
            let mut levels_map: BTreeMap<String, Vec<String>> = BTreeMap::new();

            for z_entry in fs::read_dir(&set_path)? {
                let z_entry = z_entry?;
                let z_path = z_entry.path();
                if !z_path.is_dir() {
                    continue;
                }
                let z_name = z_entry.file_name().to_string_lossy().into_owned();
                if z_name.is_empty() || !z_name.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }

                let mut keys = Vec::new();
                for f_entry in fs::read_dir(&z_path)? {
                    let f_entry = f_entry?;
                    let fname = f_entry.file_name().to_string_lossy().into_owned();
                    if let Some((y, x)) = parse_tile_filename(&fname) {
                        keys.push(format!("{y},{x}"));
                    }
                }
                keys.sort();
                levels_map.insert(z_name, keys);
            }
            manifest.insert(set_name, levels_map);
        }
    } else {
        eprintln!(
            "[build-data] tiles dir not found ({}); run \"npm run assets:link\" first. Map will render without basemap.",
            tiles_dir.display()
        );
    }

    let tile_count: usize = manifest
        .values()
        .flat_map(|levels| levels.values())
        .map(Vec::len)
        .sum();
    let tilesets = manifest.len();

    fs::write(
        out_dir.join("tile-manifest.json"),
        serde_json::to_string(&manifest)?,
    )?;

    let datasets = serde_json::json!({
        "generatedAt": iso8601_utc_now(),
        "sources": source_entries,
    });
    fs::write(
        out_dir.join("datasets.json"),
        serde_json::to_string_pretty(&datasets)?,
    )?;

    println!(
        "[build-data] {} tilesets={} tiles={}",
        summary_parts.join(" "),
        tilesets,
        tile_count
    );
    Ok(())
}

fn run_stats(data: Option<&Path>, as_json: bool) -> Result<()> {
    let landmarks = load_landmarks(data)?;
    let stats = landmark_stats(&landmarks);
    if as_json {
        println!("{}", serde_json::to_string_pretty(&stats)?);
        return Ok(());
    }

    println!("Total landmarks : {}", stats.total);
    println!("Positioned (IG) : {}", stats.positioned);
    println!(
        "IRL status      : confirmed={}, unconfirmed={}, unknown={}",
        stats.by_status.confirmed, stats.by_status.unconfirmed, stats.by_status.unknown
    );

    let mut tags: Vec<(&String, &usize)> = stats.by_tag.iter().collect();
    tags.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    println!("Top tags:");
    for (tag, count) in tags.into_iter().take(10) {
        println!("  {tag:<18} {count}");
    }

    let mut districts: Vec<(&String, &usize)> = stats.by_district.iter().collect();
    districts.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    println!("Top districts:");
    for (district, count) in districts.into_iter().take(10) {
        println!("  {district:<24} {count}");
    }
    Ok(())
}

fn run_search(
    query: &str,
    tags: &[String],
    status: Option<StatusArg>,
    positioned: bool,
    limit: usize,
    data: Option<&Path>,
    as_json: bool,
) -> Result<()> {
    let landmarks = load_landmarks(data)?;
    let statuses: Vec<IrlStatus> = status.map(Into::into).into_iter().collect();
    let filter = LandmarkFilter {
        text: if query.trim().is_empty() {
            None
        } else {
            Some(query.to_string())
        },
        tags: if tags.is_empty() {
            None
        } else {
            Some(tags.to_vec())
        },
        statuses: if statuses.is_empty() {
            None
        } else {
            Some(statuses)
        },
        districts: None,
        positioned_only: positioned,
    };
    let matches: Vec<Landmark> = filter_landmarks(&landmarks, &filter)
        .into_iter()
        .take(limit)
        .collect();

    if as_json {
        println!("{}", serde_json::to_string_pretty(&matches)?);
        return Ok(());
    }

    if matches.is_empty() {
        println!("No matching landmarks.");
        return Ok(());
    }
    for l in matches {
        let coords =
            l.ig.map(|p| format!("({:.0}, {:.0})", p.x, p.y))
                .unwrap_or_else(|| "(unpositioned)".to_string());
        println!("{:<5} {:<32} {:<20} {}", l.id, l.name, coords, l.ig_address);
    }
    Ok(())
}

fn run_predict(lat: f64, lon: f64, k: usize, data: Option<&Path>, as_json: bool) -> Result<()> {
    let landmarks = load_landmarks(data)?;
    let anchors: Vec<Anchor> = landmarks
        .iter()
        .filter(|l| l.irl_status == IrlStatus::Confirmed)
        .filter_map(|l| {
            Some(Anchor {
                irl: l.irl?,
                ig: l.ig?,
            })
        })
        .collect();

    let model =
        build_irl_to_ig_model(&anchors).context("no confirmed IRL+IG anchor landmarks found")?;
    let pred =
        predict_ig(&model, LatLon { lat, lon }, k).context("failed to fit local affine model")?;

    if as_json {
        println!("{}", serde_json::to_string_pretty(&pred)?);
    } else {
        println!(
            "Predicted IG: ({:.2}, {:.2})  rms={:.2} m  neighbours={}",
            pred.point.x, pred.point.y, pred.rms, pred.neighbours
        );
    }
    Ok(())
}

fn parse_ray(spec: &str) -> Result<Ray2D> {
    let parts: Vec<&str> = spec.split(',').collect();
    if parts.len() != 3 && parts.len() != 4 {
        bail!("invalid ray '{spec}': expected x,y,bearing[,weight]");
    }
    let x: f64 = parts[0]
        .trim()
        .parse()
        .with_context(|| format!("invalid x in ray '{spec}'"))?;
    let y: f64 = parts[1]
        .trim()
        .parse()
        .with_context(|| format!("invalid y in ray '{spec}'"))?;
    let bearing: f64 = parts[2]
        .trim()
        .parse()
        .with_context(|| format!("invalid bearing in ray '{spec}'"))?;
    let origin = WorldPoint { x, y };
    if parts.len() == 4 {
        let weight: f64 = parts[3]
            .trim()
            .parse()
            .with_context(|| format!("invalid weight in ray '{spec}'"))?;
        Ok(Ray2D::weighted(origin, bearing, weight))
    } else {
        Ok(Ray2D::new(origin, bearing))
    }
}

fn run_triangulate(ray_specs: &[String], as_json: bool) -> Result<()> {
    let rays: Vec<Ray2D> = ray_specs
        .iter()
        .map(|s| parse_ray(s))
        .collect::<Result<_>>()?;
    let res = triangulate(&rays).context("rays are parallel or insufficient to triangulate")?;

    if as_json {
        println!("{}", serde_json::to_string_pretty(&res)?);
    } else {
        println!(
            "Triangulated IG: ({:.2}, {:.2})  rms={:.2} m",
            res.point.x, res.point.y, res.rms
        );
    }
    Ok(())
}

fn run_export_geojson(out: Option<&Path>, data: Option<&Path>) -> Result<()> {
    let landmarks = load_landmarks(data)?;
    let geojson = to_geojson(&landmarks);
    let serialized = serde_json::to_string_pretty(&geojson)?;
    match out {
        Some(path) => {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)?;
                }
            }
            fs::write(path, serialized)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        None => println!("{serialized}"),
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::BuildData { root, tiles } => run_build_data(&root, tiles.as_deref()),
        Commands::BuildSolData { root } => sol::run_build_sol_data(&root),
        Commands::Stats { data, json } => run_stats(data.as_deref(), json),
        Commands::Search {
            query,
            tags,
            status,
            positioned,
            limit,
            data,
            json,
        } => run_search(
            &query,
            &tags,
            status,
            positioned,
            limit,
            data.as_deref(),
            json,
        ),
        Commands::Predict {
            lat,
            lon,
            k,
            data,
            json,
        } => run_predict(lat, lon, k, data.as_deref(), json),
        Commands::Triangulate { rays, json } => run_triangulate(&rays, json),
        Commands::ExportGeojson { out, data } => {
            run_export_geojson(out.as_deref(), data.as_deref())
        }
        Commands::Route {
            x1,
            y1,
            x2,
            y2,
            k,
            data,
            json,
        } => run_route(x1, y1, x2, y2, k, data.as_deref(), json),
        Commands::SolveCamera {
            width,
            height,
            fov,
            obs,
            json,
        } => run_solve_camera(width, height, fov, &obs, json),
        Commands::Rage(args) => rage::run_rage(args),
        Commands::Phone(args) => phone::run_phone(args),
    }
}

fn run_route(
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    k: usize,
    data: Option<&Path>,
    as_json: bool,
) -> Result<()> {
    let landmarks = load_landmarks(data)?;
    let nodes: Vec<WorldPoint> = landmarks.into_iter().filter_map(|l| l.ig).collect();
    let res = v04::plan_route(&nodes, x1, y1, x2, y2, k);
    if as_json {
        println!("{}", serde_json::to_string_pretty(&res)?);
    } else {
        println!(
            "Route: {} waypoints, distance={:.1} m, drive={:.2} min, fly={:.2} min",
            res.waypoints.len(),
            res.distance_m,
            res.drive_min,
            res.fly_min
        );
    }
    Ok(())
}

fn parse_camera_obs(spec: &str) -> Result<v04::CameraObs> {
    let parts: Vec<&str> = spec.split(',').collect();
    if parts.len() != 5 {
        bail!("invalid observation '{spec}': expected world_x,world_y,world_z,px,py");
    }
    let parse_f = |idx: usize, name: &str| -> Result<f64> {
        parts[idx]
            .trim()
            .parse()
            .with_context(|| format!("invalid {name} in '{spec}'"))
    };
    Ok(v04::CameraObs {
        world_x: parse_f(0, "world_x")?,
        world_y: parse_f(1, "world_y")?,
        world_z: parse_f(2, "world_z")?,
        px: parse_f(3, "px")?,
        py: parse_f(4, "py")?,
    })
}

fn run_solve_camera(
    width: f64,
    height: f64,
    fov: Option<f64>,
    obs_specs: &[String],
    as_json: bool,
) -> Result<()> {
    let obs: Vec<v04::CameraObs> = obs_specs
        .iter()
        .map(|s| parse_camera_obs(s))
        .collect::<Result<_>>()?;
    let pose = v04::solve_camera_pose(&obs, width, height, fov)
        .context("failed to solve camera pose (need >= 3 valid observations)")?;
    if as_json {
        println!("{}", serde_json::to_string_pretty(&pose)?);
    } else {
        println!(
            "CameraPose: ({:.2}, {:.2}, {:.2}) yaw={:.2}° pitch={:.2}° fov={:.1}° rmse={:.2} px",
            pose.x, pose.y, pose.z, pose.yaw_deg, pose.pitch_deg, pose.fov_deg, pose.rmse_px
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_negative_predict_args() {
        let cli =
            Cli::try_parse_from(["leonida", "predict", "25.898996", "-80.1232991", "-k", "12"])
                .expect("should parse negative longitude");
        match cli.command {
            Commands::Predict { lat, lon, k, .. } => {
                assert!((lat - 25.898996).abs() < 1e-6);
                assert!((lon - (-80.1232991)).abs() < 1e-6);
                assert_eq!(k, 12);
            }
            _ => panic!("expected Predict"),
        }
    }

    #[test]
    fn parse_negative_triangulate_rays() {
        let cli = Cli::try_parse_from(["leonida", "triangulate", "-100,200,90", "100,0,0"])
            .expect("should parse negative ray coords");
        match cli.command {
            Commands::Triangulate { rays, .. } => {
                assert_eq!(rays.len(), 2);
                let parsed: Vec<Ray2D> = rays.iter().map(|s| parse_ray(s).unwrap()).collect();
                let res = triangulate(&parsed).unwrap();
                assert!((res.point.x - 100.0).abs() < 1e-6);
                assert!((res.point.y - 200.0).abs() < 1e-6);
                assert!(res.rms < 1e-6);
            }
            _ => panic!("expected Triangulate"),
        }
    }

    #[test]
    fn parse_real_gta6map_sample_and_stats() {
        let sample = r#"[
            ["b1", "?, Vice Beach", [1340,2830], 0, "The Ritz-Carlton Bal Harbour, 10295 Collins Ave", [25.8989960,-80.1232991], 0, ["hotel"], "166624", 1752839001.0],
            ["b2", "?, Vice Beach", [], 0, "?", [], 0, ["misc"], "bad", 1752839001.0]
        ]"#;
        let rows: Vec<Gta6MapTuple> = serde_json::from_str(sample).unwrap();
        let list: Vec<Landmark> = rows
            .iter()
            .map(|r| parse_gta6map_tuple(r, "gta6map.github.io"))
            .collect();
        let stats = landmark_stats(&list);
        assert_eq!(stats.total, 2);
        assert_eq!(stats.positioned, 1);
        assert_eq!(stats.by_status.confirmed, 1);
        assert_eq!(stats.by_status.unknown, 1);
        assert_eq!(list[0].name, "The Ritz-Carlton Bal Harbour");
        assert_eq!(list[1].name, "Unknown b2");
        assert_eq!(list[1].color, "888888");
    }

    #[test]
    fn geojson_export_filters_positioned() {
        let rows: Vec<Gta6MapTuple> = serde_json::from_str(
            r#"[
                ["b1", "Tower, Downtown", [100,200], 1, "Miami Tower", [25.77,-80.19], 0, ["office"], "aabbcc", 1.0],
                ["b2", "?, Downtown", [], 0, "?", [], 0, [], "112233", 2.0]
            ]"#,
        )
        .unwrap();
        let list: Vec<Landmark> = rows
            .iter()
            .map(|r| parse_gta6map_tuple(r, "gta6map.github.io"))
            .collect();
        let gj = to_geojson(&list);
        assert_eq!(gj.features.len(), 1);
        assert_eq!(gj.features[0].id, "b1");
        assert_eq!(gj.features[0].properties.color, "#aabbcc");
    }

    #[test]
    fn build_data_on_temp_dir() {
        let base = std::env::temp_dir().join(format!(
            "leonida-cli-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let raw = base.join("data/raw");
        let tiles_z = base.join("apps/map/public/tiles/dupzor,51/0");
        fs::create_dir_all(&raw).unwrap();
        fs::create_dir_all(&tiles_z).unwrap();
        fs::write(raw.join("gta6map-landmarks.json"), "[]").unwrap();
        fs::write(
            raw.join("gtamaplib-vc-landmarks.json"),
            r#"{"schema":"gtamaplib-vvc-landmarks-v1","landmarks":[]}"#,
        )
        .unwrap();
        fs::write(tiles_z.join("0,0,0.jpg"), b"").unwrap();

        run_build_data(&base, None).unwrap();

        let manifest: Value = serde_json::from_str(
            &fs::read_to_string(base.join("apps/map/public/data/tile-manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["dupzor,51"]["0"][0], "0,0");
        assert!(base.join("apps/map/public/data/datasets.json").exists());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn build_sol_data_on_temp_dir() {
        let base = std::env::temp_dir().join(format!(
            "leonida-sol-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let raw = base.join("data/raw");
        fs::create_dir_all(&raw).unwrap();
        fs::write(
            raw.join("gta6map-landmarks.json"),
            r#"[
                ["b1", "?, Vice Beach", [1340,2830], 0, "The Ritz-Carlton, FL", [25.89,-80.12], 0, ["hotel"], "166624", 1.0],
                ["b2", "?, Port Gellhorn", [500,600], 0, "?", [], 0, ["industrial"], "888888", 2.0]
            ]"#,
        )
        .unwrap();
        fs::write(
            raw.join("gtamaplib-vc-landmarks.json"),
            r#"{"schema":"gtamaplib-vvc-landmarks-v1","landmarks":[{"id":"L1","name":"Tower","xyz":[10,20,30]}]}"#,
        )
        .unwrap();

        sol::run_build_sol_data(&base).unwrap();

        let out = base.join("apps/map/public/data");
        for name in [
            "markers.json",
            "trailer1.json",
            "trailer2.json",
            "trailer3.json",
            "leaks.json",
            "screenshots.json",
            "el_screenshots.json",
            "news.json",
            "database_changelog.json",
            "content.json",
            "credits.json",
            "supporters.json",
            "backgrounds.json",
            "lang.json",
            "seo.json",
        ] {
            assert!(out.join(name).exists(), "missing {name}");
        }

        let markers: Value =
            serde_json::from_str(&fs::read_to_string(out.join("markers.json")).unwrap()).unwrap();
        assert_eq!(markers.as_array().unwrap().len(), 3);

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn parse_route_and_solve_camera_cli() {
        let cli =
            Cli::try_parse_from(["leonida", "route", "-100", "200", "500", "-300", "-k", "5"])
                .expect("should parse negative route coords");
        match cli.command {
            Commands::Route {
                x1, y1, x2, y2, k, ..
            } => {
                assert_eq!((x1, y1, x2, y2, k), (-100.0, 200.0, 500.0, -300.0, 5));
            }
            _ => panic!("expected Route"),
        }

        let cli = Cli::try_parse_from([
            "leonida",
            "solve-camera",
            "--width",
            "1280",
            "--height",
            "720",
            "-100,500,10,418.3,360",
            "0,500,50,640,271.32",
            "120,600,20,861.7,341.52",
        ])
        .expect("should parse solve-camera");
        match cli.command {
            Commands::SolveCamera { obs, .. } => {
                let parsed: Vec<v04::CameraObs> =
                    obs.iter().map(|s| parse_camera_obs(s).unwrap()).collect();
                let pose = v04::solve_camera_pose(&parsed, 1280.0, 720.0, Some(60.0)).unwrap();
                assert!(pose.rmse_px < 10.0);
            }
            _ => panic!("expected SolveCamera"),
        }
    }

    #[test]
    fn parse_rage_subcommands_and_helpers() {
        let cli = Cli::try_parse_from(["leonida", "rage", "joaat", "adder", "--size", "4096"])
            .expect("should parse rage joaat");
        match cli.command {
            Commands::Rage(args) => match args.command {
                rage::RageCommands::Joaat { name, size, .. } => {
                    assert_eq!(name, "adder");
                    assert_eq!(size, Some(4096));
                    assert_eq!(rage::joaat(&name), 0xB779_A091);
                }
                _ => panic!("expected Joaat"),
            },
            _ => panic!("expected Rage"),
        }

        let rpf_bytes = rage::parse_hex_bytes("5250463710000000000200004F50454E").unwrap();
        let rpf = rage::parse_rpf_header(&rpf_bytes).unwrap();
        assert_eq!(rpf.version, "RPF7");
        assert_eq!(rpf.encryption, "Open");

        let rsc_bytes = rage::parse_hex_bytes("525343370D0000000020000000400000").unwrap();
        let rsc = rage::parse_rsc7_header(&rsc_bytes).unwrap();
        assert_eq!(rsc.version, 13);
        assert!(rsc.system_size > 0);
    }

    #[test]
    fn phone_stats_on_temp_dir() {
        let base = std::env::temp_dir().join(format!(
            "leonida-phone-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let wasm_dir = base.join("apps/phone/src/wasm");
        let data_dir = base.join("apps/map/public/data");
        fs::create_dir_all(&wasm_dir).unwrap();
        fs::create_dir_all(&data_dir).unwrap();
        fs::write(wasm_dir.join("leonida_wasm.js"), "// js").unwrap();
        fs::write(wasm_dir.join("leonida_wasm_bg.wasm"), b"wasm").unwrap();
        fs::write(data_dir.join("markers.json"), "[]").unwrap();
        fs::write(
            data_dir.join("gta6map-landmarks.json"),
            r#"[["b1","?, Vice Beach",[100,200],0,"Hotel",[25.8,-80.1],0,["hotel"],"112233",1.0]]"#,
        )
        .unwrap();

        let report = phone::collect_phone_stats(&base, None).unwrap();
        assert!(report.wasm_ready);
        assert_eq!(report.wasm_bytes, 4);
        assert_eq!(report.sol_datasets_present, 1);
        assert_eq!(report.total_landmarks, 1);
        assert_eq!(report.positioned_landmarks, 1);

        let _ = fs::remove_dir_all(&base);
    }
}
