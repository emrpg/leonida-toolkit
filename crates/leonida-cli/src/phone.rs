use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use leonida_core::{landmark_stats, parse_gta6map_tuple, Gta6MapTuple};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Args, Debug)]
pub struct PhoneArgs {
    #[command(subcommand)]
    pub command: PhoneCommands,
}

#[derive(Subcommand, Debug)]
pub enum PhoneCommands {
    /// Validate LeonidaOS phone assets/WASM and print companion dataset statistics
    Stats {
        /// Workspace root directory
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Optional path to gta6map-landmarks.json
        #[arg(long)]
        data: Option<PathBuf>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoneStatsReport {
    pub wasm_ready: bool,
    pub wasm_bytes: u64,
    pub sol_datasets_present: usize,
    pub total_landmarks: usize,
    pub positioned_landmarks: usize,
    pub districts: usize,
    pub top_categories: Vec<(String, usize)>,
}

pub fn collect_phone_stats(root: &Path, data_opt: Option<&Path>) -> Result<PhoneStatsReport> {
    let wasm_js = root.join("apps/phone/src/wasm/leonida_wasm.js");
    let wasm_bin = root.join("apps/phone/src/wasm/leonida_wasm_bg.wasm");
    let wasm_ready = wasm_js.exists() && wasm_bin.exists();
    let wasm_bytes = fs::metadata(&wasm_bin).map(|m| m.len()).unwrap_or(0);

    let data_dir = root.join("apps/map/public/data");
    let sol_files = [
        "markers.json",
        "trailer1.json",
        "trailer2.json",
        "trailer3.json",
        "leaks.json",
        "screenshots.json",
        "el_screenshots.json",
    ];
    let sol_datasets_present = sol_files
        .iter()
        .filter(|f| data_dir.join(f).exists())
        .count();

    let landmarks_path = match data_opt {
        Some(p) => p.to_path_buf(),
        None => {
            let public = data_dir.join("gta6map-landmarks.json");
            if public.exists() {
                public
            } else {
                root.join("data/raw/gta6map-landmarks.json")
            }
        }
    };

    let raw = fs::read_to_string(&landmarks_path)
        .with_context(|| format!("failed to read {}", landmarks_path.display()))?;
    let rows: Vec<Gta6MapTuple> = serde_json::from_str(&raw)?;
    let landmarks: Vec<_> = rows
        .iter()
        .map(|r| parse_gta6map_tuple(r, "gta6map.github.io"))
        .collect();
    let stats = landmark_stats(&landmarks);

    let mut cats: Vec<(String, usize)> = stats
        .by_tag
        .into_iter()
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .collect();
    cats.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    cats.truncate(8);

    Ok(PhoneStatsReport {
        wasm_ready,
        wasm_bytes,
        sol_datasets_present,
        total_landmarks: stats.total,
        positioned_landmarks: stats.positioned,
        districts: stats.by_district.len(),
        top_categories: cats,
    })
}

pub fn run_phone(args: PhoneArgs) -> Result<()> {
    match args.command {
        PhoneCommands::Stats { root, data, json } => {
            let report = collect_phone_stats(&root, data.as_deref())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "Phone WASM      : {} ({} bytes)",
                    if report.wasm_ready {
                        "ready"
                    } else {
                        "missing"
                    },
                    report.wasm_bytes
                );
                println!(
                    "SOL datasets    : {}/7 present",
                    report.sol_datasets_present
                );
                println!(
                    "Landmarks       : {} total, {} positioned ({} districts)",
                    report.total_landmarks, report.positioned_landmarks, report.districts
                );
                println!("Top categories  :");
                for (cat, count) in &report.top_categories {
                    println!("  {cat:<18} {count}");
                }
            }
        }
    }
    Ok(())
}
