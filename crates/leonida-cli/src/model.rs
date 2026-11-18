use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const EARTH_R: f64 = 6_371_008.8;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WorldPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

pub fn bearing_to_vector(bearing_deg: f64) -> WorldPoint {
    let r = bearing_deg.to_radians();
    WorldPoint {
        x: r.sin(),
        y: r.cos(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IrlStatus {
    Confirmed,
    Unconfirmed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Landmark {
    pub id: String,
    pub ig_address: String,
    pub name: String,
    pub district: Option<String>,
    pub ig: Option<WorldPoint>,
    pub irl_address: String,
    pub irl: Option<LatLon>,
    pub irl_status: IrlStatus,
    pub tags: Vec<String>,
    pub color: String,
    pub edited_at: f64,
    pub ig_photo: bool,
    pub irl_photo: bool,
    pub source: String,
}

#[derive(Debug, Deserialize)]
pub struct Gta6MapTuple(
    pub String,
    pub String,
    pub Vec<f64>,
    pub u8,
    pub String,
    pub Vec<f64>,
    pub u8,
    pub Vec<String>,
    pub String,
    pub f64,
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
            let building = if !b.is_empty() && b != "?" {
                Some(b.to_string())
            } else {
                None
            };
            let district = if !d.is_empty() {
                Some(d.to_string())
            } else {
                None
            };
            (building, district)
        }
    }
}

fn irl_name(irl_address: &str) -> String {
    irl_address
        .split(',')
        .next()
        .map(str::trim)
        .unwrap_or(irl_address)
        .to_string()
}

pub fn parse_gta6map_tuple(row: Gta6MapTuple, source: &str) -> Landmark {
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

    let irl_address = if irl_raw.len() > 1 && irl_raw.ends_with('?') {
        irl_raw[..irl_raw.len() - 1].to_string()
    } else {
        irl_raw
    };

    let (building, district) = split_address(&ig_address);
    let name = building.unwrap_or_else(|| {
        if irl_status == IrlStatus::Unknown {
            format!("Unknown {id}")
        } else {
            irl_name(&irl_address)
        }
    });

    let ig = if ig_c.len() >= 2 {
        Some(WorldPoint {
            x: ig_c[0],
            y: ig_c[1],
        })
    } else {
        None
    };

    let irl = if irl_c.len() >= 2 {
        Some(LatLon {
            lat: irl_c[0],
            lon: irl_c[1],
        })
    } else {
        None
    };

    let valid_color = color.len() == 6 && color.chars().all(|c| c.is_ascii_hexdigit());
    let norm_color = if valid_color {
        color.to_ascii_lowercase()
    } else {
        "888888".to_string()
    };

    Landmark {
        id,
        ig_address,
        name,
        district,
        ig,
        irl_address,
        irl,
        irl_status,
        tags,
        color: norm_color,
        edited_at,
        ig_photo: ig_photo != 0,
        irl_photo: irl_photo != 0,
        source: source.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusCounts {
    pub confirmed: usize,
    pub unconfirmed: usize,
    pub unknown: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LandmarkStats {
    pub total: usize,
    pub positioned: usize,
    pub by_status: StatusCounts,
    pub by_tag: BTreeMap<String, usize>,
    pub by_district: BTreeMap<String, usize>,
}

pub fn landmark_stats(list: &[Landmark]) -> LandmarkStats {
    let mut stats = LandmarkStats {
        total: list.len(),
        positioned: 0,
        by_status: StatusCounts {
            confirmed: 0,
            unconfirmed: 0,
            unknown: 0,
        },
        by_tag: BTreeMap::new(),
        by_district: BTreeMap::new(),
    };

    for l in list {
        if l.ig.is_some() {
            stats.positioned += 1;
        }
        match l.irl_status {
            IrlStatus::Confirmed => stats.by_status.confirmed += 1,
            IrlStatus::Unconfirmed => stats.by_status.unconfirmed += 1,
            IrlStatus::Unknown => stats.by_status.unknown += 1,
        }
        for t in &l.tags {
            *stats.by_tag.entry(t.clone()).or_insert(0) += 1;
        }
        let d = l.district.as_deref().unwrap_or("(none)").to_string();
        *stats.by_district.entry(d).or_insert(0) += 1;
    }

    stats
}

pub fn project_local(p: LatLon, origin: LatLon) -> WorldPoint {
    let r = std::f64::consts::PI / 180.0;
    WorldPoint {
        x: (p.lon - origin.lon) * r * EARTH_R * (origin.lat * r).cos(),
        y: (p.lat - origin.lat) * r * EARTH_R,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Affine2D {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

pub fn apply_affine(t: &Affine2D, p: WorldPoint) -> WorldPoint {
    WorldPoint {
        x: t.a * p.x + t.b * p.y + t.c,
        y: t.d * p.x + t.e * p.y + t.f,
    }
}

fn det3(q: &[[f64; 3]; 3]) -> f64 {
    q[0][0] * (q[1][1] * q[2][2] - q[1][2] * q[2][1])
        - q[0][1] * (q[1][0] * q[2][2] - q[1][2] * q[2][0])
        + q[0][2] * (q[1][0] * q[2][1] - q[1][1] * q[2][0])
}

fn solve3(m: &[[f64; 3]; 3], v: &[f64; 3]) -> Option<[f64; 3]> {
    let d = det3(m);
    if d.abs() < 1e-12 {
        return None;
    }
    let mut out = [0.0; 3];
    for c in 0..3 {
        let mut mc = *m;
        for i in 0..3 {
            mc[i][c] = v[i];
        }
        out[c] = det3(&mc) / d;
    }
    Some(out)
}

#[derive(Debug, Clone, Copy)]
pub struct PointPair {
    pub from: WorldPoint,
    pub to: WorldPoint,
    pub weight: f64,
}

pub fn fit_affine(pairs: &[PointPair]) -> Option<Affine2D> {
    if pairs.len() < 3 {
        return None;
    }
    let mut n = [[0.0; 3]; 3];
    let mut bx = [0.0; 3];
    let mut by = [0.0; 3];
    for p in pairs {
        let row = [p.from.x, p.from.y, 1.0];
        for i in 0..3 {
            for j in 0..3 {
                n[i][j] += p.weight * row[i] * row[j];
            }
            bx[i] += p.weight * row[i] * p.to.x;
            by[i] += p.weight * row[i] * p.to.y;
        }
    }
    let sx = solve3(&n, &bx)?;
    let sy = solve3(&n, &by)?;
    Some(Affine2D {
        a: sx[0],
        b: sx[1],
        c: sx[2],
        d: sy[0],
        e: sy[1],
        f: sy[2],
    })
}

pub struct IrlToIgModel {
    pub origin: LatLon,
    pub pairs: Vec<(WorldPoint, WorldPoint)>,
}

pub fn build_irl_to_ig_model(anchors: &[(LatLon, WorldPoint)]) -> Option<IrlToIgModel> {
    if anchors.is_empty() {
        return None;
    }
    let n = anchors.len() as f64;
    let origin = LatLon {
        lat: anchors.iter().map(|a| a.0.lat).sum::<f64>() / n,
        lon: anchors.iter().map(|a| a.0.lon).sum::<f64>() / n,
    };
    let pairs = anchors
        .iter()
        .map(|a| (project_local(a.0, origin), a.1))
        .collect();
    Some(IrlToIgModel { origin, pairs })
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Prediction {
    pub point: WorldPoint,
    pub rms: f64,
    pub neighbours: usize,
}

pub fn predict_ig(model: &IrlToIgModel, irl: LatLon, k: usize) -> Option<Prediction> {
    if model.pairs.len() < 3 {
        return None;
    }
    let q = project_local(irl, model.origin);
    let mut ranked: Vec<(WorldPoint, WorldPoint, f64)> = model
        .pairs
        .iter()
        .map(|&(from, to)| (from, to, (from.x - q.x).hypot(from.y - q.y)))
        .collect();
    ranked.sort_by(|a, b| a.2.total_cmp(&b.2));
    ranked.truncate(k.max(3));

    let scale = ranked
        .last()
        .map(|r| if r.2 == 0.0 { 1.0 } else { r.2 })
        .unwrap_or(1.0);

    let weighted: Vec<PointPair> = ranked
        .iter()
        .map(|&(from, to, d)| PointPair {
            from,
            to,
            weight: 1.0 / (0.05 + (d / scale).powi(2)),
        })
        .collect();

    let t = fit_affine(&weighted)?;
    let se: f64 = weighted
        .iter()
        .map(|w| {
            let r = apply_affine(&t, w.from);
            (r.x - w.to.x).powi(2) + (r.y - w.to.y).powi(2)
        })
        .sum();

    Some(Prediction {
        point: apply_affine(&t, q),
        rms: (se / weighted.len() as f64).sqrt(),
        neighbours: weighted.len(),
    })
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Ray2D {
    pub origin: WorldPoint,
    pub bearing: f64,
    pub weight: f64,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Triangulation {
    pub point: WorldPoint,
    pub rms: f64,
}

pub fn triangulate(rays: &[Ray2D]) -> Option<Triangulation> {
    if rays.len() < 2 {
        return None;
    }
    let (mut a11, mut a12, mut a22, mut b1, mut b2) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for r in rays {
        let d = bearing_to_vector(r.bearing);
        let w = r.weight;
        let m11 = 1.0 - d.x * d.x;
        let m12 = -d.x * d.y;
        let m22 = 1.0 - d.y * d.y;
        a11 += w * m11;
        a12 += w * m12;
        a22 += w * m22;
        b1 += w * (m11 * r.origin.x + m12 * r.origin.y);
        b2 += w * (m12 * r.origin.x + m22 * r.origin.y);
    }
    let det = a11 * a22 - a12 * a12;
    if det.abs() < 1e-9 {
        return None;
    }
    let point = WorldPoint {
        x: (a22 * b1 - a12 * b2) / det,
        y: (a11 * b2 - a12 * b1) / det,
    };
    let se: f64 = rays
        .iter()
        .map(|r| {
            let d = bearing_to_vector(r.bearing);
            let vx = point.x - r.origin.x;
            let vy = point.y - r.origin.y;
            (vx * d.y - vy * d.x).powi(2)
        })
        .sum();
    Some(Triangulation {
        point,
        rms: (se / rays.len() as f64).sqrt(),
    })
}

pub struct LandmarkFilter<'a> {
    pub text: Option<&'a str>,
    pub tags: &'a [String],
    pub statuses: &'a [IrlStatus],
    pub positioned_only: bool,
}

pub fn filter_landmarks<'a>(list: &'a [Landmark], f: &LandmarkFilter<'_>) -> Vec<&'a Landmark> {
    let terms: Vec<String> = f
        .text
        .map(|t| {
            t.to_lowercase()
                .split_whitespace()
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    list.iter()
        .filter(|l| {
            if f.positioned_only && l.ig.is_none() {
                return false;
            }
            if !f.tags.is_empty() && !l.tags.iter().any(|t| f.tags.contains(t)) {
                return false;
            }
            if !f.statuses.is_empty() && !f.statuses.contains(&l.irl_status) {
                return false;
            }
            if !terms.is_empty() {
                let hay = format!(
                    "{}\n{}\n{}\n{}",
                    l.id,
                    l.ig_address,
                    l.irl_address,
                    l.tags.join(" ")
                )
                .to_lowercase();
                if !terms.iter().all(|term| hay.contains(term.as_str())) {
                    return false;
                }
            }
            true
        })
        .collect()
}

pub fn to_geojson(list: &[Landmark]) -> serde_json::Value {
    let features: Vec<serde_json::Value> = list
        .iter()
        .filter_map(|l| {
            let ig = l.ig?;
            Some(serde_json::json!({
                "type": "Feature",
                "id": l.id,
                "geometry": {
                    "type": "Point",
                    "coordinates": [ig.x, ig.y],
                },
                "properties": {
                    "name": l.name,
                    "igAddress": l.ig_address,
                    "district": l.district,
                    "irlAddress": l.irl_address,
                    "irlStatus": l.irl_status,
                    "irl": l.irl,
                    "tags": l.tags,
                    "color": format!("#{}", l.color),
                    "source": l.source,
                }
            }))
        })
        .collect();

    serde_json::json!({
        "type": "FeatureCollection",
        "crs": {
            "type": "name",
            "properties": { "name": "urn:leonida:world-metres" }
        },
        "features": features,
    })
}
