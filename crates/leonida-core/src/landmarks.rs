//! Landmark parsing and summary statistics for gta6map & gtamaplib-vc datasets.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::coords::{LatLon, WorldPoint, WorldPoint3};
use crate::error::CoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IrlStatus {
    Confirmed,
    Unconfirmed,
    Unknown,
}

/// Normalised landmark record used across all Leonida Toolkit apps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Landmark {
    pub id: String,
    /// In-game address, e.g. `"?, Vice Beach"` (unknown building name in district Vice Beach).
    pub ig_address: String,
    /// Display name: building part of the in-game address, or the real-world name when unknown.
    pub name: String,
    pub district: Option<String>,
    pub ig: Option<WorldPoint>,
    pub irl_address: String,
    pub irl: Option<LatLon>,
    pub irl_status: IrlStatus,
    pub tags: Vec<String>,
    /// 6-digit hex colour without `'#'`.
    pub color: String,
    /// Unix seconds.
    pub edited_at: f64,
    pub ig_photo: bool,
    pub irl_photo: bool,
    pub source: String,
}

/// 3D landmark from gtamaplib-vc (triangulated position incl. height).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Landmark3D {
    pub id: String,
    pub name: String,
    pub xyz: WorldPoint3,
    pub shape: String,
    pub width: f64,
    pub height: f64,
    pub source: String,
}

/// Row tuple of gta6map.github.io `data/landmarks.json` (public domain).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gta6MapTuple(
    pub String,      // id
    pub String,      // igAddress
    pub Vec<f64>,    // igCoordinates
    pub i64,         // igPhoto
    pub String,      // irlAddress
    pub Vec<f64>,    // irlCoordinates
    pub i64,         // irlPhoto
    pub Vec<String>, // tags
    pub String,      // color
    pub f64,         // editedAt
);

fn split_address(ig_address: &str) -> (Option<String>, Option<String>) {
    match ig_address.rfind(',') {
        None => {
            let trimmed = ig_address.trim();
            let building = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            };
            (building, None)
        }
        Some(idx) => {
            let b = ig_address[..idx].trim();
            let d = ig_address[idx + 1..].trim();
            let building = if b.is_empty() || b == "?" {
                None
            } else {
                Some(b.to_string())
            };
            let district = if d.is_empty() {
                None
            } else {
                Some(d.to_string())
            };
            (building, district)
        }
    }
}

fn irl_name(irl_address: &str) -> &str {
    irl_address
        .split(',')
        .next()
        .map(str::trim)
        .unwrap_or(irl_address)
}

/// Parse a single `gta6map.github.io` tuple row into a normalised [`Landmark`].
pub fn parse_gta6map_tuple(row: &Gta6MapTuple, source: &str) -> Landmark {
    let Gta6MapTuple(
        id,
        ig_address,
        ig_c,
        ig_photo,
        irl_raw,
        irl_c,
        irl_photo,
        tags,
        color,
        edited_at,
    ) = row;

    let irl_status = if irl_raw == "?" {
        IrlStatus::Unknown
    } else if irl_raw.ends_with('?') {
        IrlStatus::Unconfirmed
    } else {
        IrlStatus::Confirmed
    };

    let irl_address = if irl_raw.len() > 1 {
        irl_raw.strip_suffix('?').unwrap_or(irl_raw).to_string()
    } else {
        irl_raw.clone()
    };

    let (building, district) = split_address(ig_address);
    let name = building.unwrap_or_else(|| {
        if irl_status == IrlStatus::Unknown {
            format!("Unknown {id}")
        } else {
            irl_name(&irl_address).to_string()
        }
    });

    let ig = if ig_c.len() >= 2 {
        Some(WorldPoint::new(ig_c[0], ig_c[1]))
    } else {
        None
    };

    let irl = if irl_c.len() >= 2 {
        Some(LatLon::new(irl_c[0], irl_c[1]))
    } else {
        None
    };

    let norm_color = if color.len() == 6 && color.chars().all(|c| c.is_ascii_hexdigit()) {
        color.to_ascii_lowercase()
    } else {
        "888888".to_string()
    };

    Landmark {
        id: id.clone(),
        ig_address: ig_address.clone(),
        name,
        district,
        ig,
        irl_address,
        irl,
        irl_status,
        tags: tags.clone(),
        color: norm_color,
        edited_at: *edited_at,
        ig_photo: *ig_photo != 0,
        irl_photo: *irl_photo != 0,
        source: source.to_string(),
    }
}

/// Raw entry in a `gtamaplib-vc` landmarks JSON file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GtamaplibVcEntry {
    pub name: String,
    pub id: String,
    pub xyz: Vec<f64>,
    #[serde(default)]
    pub shape: Option<String>,
    #[serde(default)]
    pub width: Option<f64>,
    #[serde(default)]
    pub height: Option<f64>,
}

/// Top-level `gtamaplib-vc` landmarks JSON document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GtamaplibVcFile {
    pub schema: String,
    pub landmarks: Vec<GtamaplibVcEntry>,
}

/// Parse a `gtamaplib-vc` 3D landmark file.
pub fn parse_gtamaplib_vc(
    file: &GtamaplibVcFile,
    source: &str,
) -> Result<Vec<Landmark3D>, CoreError> {
    if !file.schema.starts_with("gtamaplib-vvc-landmarks") {
        return Err(CoreError::UnsupportedSchema(file.schema.clone()));
    }

    Ok(file
        .landmarks
        .iter()
        .filter(|l| l.xyz.len() >= 3)
        .map(|l| Landmark3D {
            id: l.id.clone(),
            name: l.name.clone(),
            xyz: WorldPoint3::new(l.xyz[0], l.xyz[1], l.xyz[2]),
            shape: l.shape.clone().unwrap_or_else(|| "box".to_string()),
            width: l.width.unwrap_or(10.0),
            height: l.height.unwrap_or(l.xyz[2]),
            source: source.to_string(),
        })
        .collect())
}

/// Natural sort key for ids like `"b12"` / `"L264"`.
pub fn id_sort_key(id: &str) -> String {
    let mut out = String::with_capacity(id.len() + 6);
    let mut digits = String::new();

    for ch in id.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else {
            if !digits.is_empty() {
                out.push_str(&format!("{digits:0>6}"));
                digits.clear();
            }
            out.push(ch);
        }
    }
    if !digits.is_empty() {
        out.push_str(&format!("{digits:0>6}"));
    }
    out
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusCounts {
    pub confirmed: usize,
    pub unconfirmed: usize,
    pub unknown: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LandmarkStats {
    pub total: usize,
    pub positioned: usize,
    pub by_status: StatusCounts,
    pub by_tag: BTreeMap<String, usize>,
    pub by_district: BTreeMap<String, usize>,
}

/// Compute summary counts for a slice of landmarks.
pub fn landmark_stats(list: &[Landmark]) -> LandmarkStats {
    let mut s = LandmarkStats {
        total: list.len(),
        ..Default::default()
    };

    for l in list {
        if l.ig.is_some() {
            s.positioned += 1;
        }
        match l.irl_status {
            IrlStatus::Confirmed => s.by_status.confirmed += 1,
            IrlStatus::Unconfirmed => s.by_status.unconfirmed += 1,
            IrlStatus::Unknown => s.by_status.unknown += 1,
        }
        for t in &l.tags {
            *s.by_tag.entry(t.clone()).or_insert(0) += 1;
        }
        let d = l.district.as_deref().unwrap_or("(none)");
        *s.by_district.entry(d.to_string()).or_insert(0) += 1;
    }

    s
}
