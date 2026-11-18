use std::collections::{BTreeMap, HashMap};
use std::f64::consts::PI;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const EARTH_R: f64 = 6_371_008.8;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct WorldPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum IrlStatus {
    Confirmed,
    Unconfirmed,
    Unknown,
}

impl IrlStatus {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "confirmed" => Some(Self::Confirmed),
            "unconfirmed" => Some(Self::Unconfirmed),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    pub edited_at: i64,
    pub ig_photo: bool,
    pub irl_photo: bool,
    pub source: String,
}

#[derive(Debug, Clone, Default)]
pub struct LandmarkFilter {
    pub text: Option<String>,
    pub tags: Vec<String>,
    pub statuses: Vec<IrlStatus>,
    pub districts: Vec<String>,
    pub positioned_only: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NearestLandmark {
    pub landmark: Landmark,
    pub distance: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusCounts {
    pub confirmed: usize,
    pub unconfirmed: usize,
    pub unknown: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LandmarkStats {
    pub total: usize,
    pub positioned: usize,
    pub by_status: StatusCounts,
    pub by_tag: BTreeMap<String, usize>,
    pub by_district: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prediction {
    pub point: WorldPoint,
    pub rms: f64,
    pub neighbours: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ray2D {
    pub origin: WorldPoint,
    pub bearing: f64,
    pub weight: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Triangulation {
    pub point: WorldPoint,
    pub rms: f64,
}

/// Core computation boundary.
///
/// Implemented locally until `crates/leonida-core` lands; can be swapped to
/// delegate directly to `leonida_core` without changing any HTTP handler.
pub trait LandmarkEngine: Send + Sync {
    fn find_by_id(&self, id: &str) -> Option<&Landmark>;
    fn filter(&self, filter: &LandmarkFilter) -> Vec<&Landmark>;
    fn nearest(&self, p: WorldPoint, n: usize) -> Vec<NearestLandmark>;
    fn stats(&self) -> &LandmarkStats;
    fn geojson(&self, filter: &LandmarkFilter) -> Value;
    fn predict_ig(&self, irl: LatLon, k: usize) -> Option<Prediction>;
    fn triangulate(&self, rays: &[Ray2D]) -> Option<Triangulation>;
}

#[derive(Debug, Clone)]
struct PointPair {
    from: WorldPoint,
    to: WorldPoint,
    weight: f64,
}

#[derive(Debug, Clone)]
struct IrlToIgModel {
    origin: LatLon,
    pairs: Vec<PointPair>,
}

#[derive(Debug, Clone)]
pub struct LocalCoreEngine {
    landmarks: Vec<Landmark>,
    index_by_id: HashMap<String, usize>,
    stats: LandmarkStats,
    irl_model: Option<IrlToIgModel>,
}

impl LocalCoreEngine {
    pub fn from_gta6map_json(raw: &str) -> Result<Self> {
        let rows: Vec<Gta6MapTuple> = serde_json::from_str(raw)?;
        let mut landmarks: Vec<Landmark> = rows
            .into_iter()
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
        let stats = compute_stats(&landmarks);
        let anchors: Vec<(LatLon, WorldPoint)> = landmarks
            .iter()
            .filter_map(|l| {
                if l.irl_status == IrlStatus::Confirmed {
                    Some((l.irl?, l.ig?))
                } else {
                    None
                }
            })
            .collect();
        let irl_model = build_irl_to_ig_model(&anchors);
        Self {
            landmarks,
            index_by_id,
            stats,
            irl_model,
        }
    }
}

impl LandmarkEngine for LocalCoreEngine {
    fn find_by_id(&self, id: &str) -> Option<&Landmark> {
        self.index_by_id.get(id).map(|&i| &self.landmarks[i])
    }

    fn filter(&self, filter: &LandmarkFilter) -> Vec<&Landmark> {
        let terms: Vec<String> = filter
            .text
            .as_deref()
            .map(|t| {
                fold_text(t)
                    .split_whitespace()
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();

        self.landmarks
            .iter()
            .filter(|l| {
                if filter.positioned_only && l.ig.is_none() {
                    return false;
                }
                if !filter.tags.is_empty() && !l.tags.iter().any(|t| filter.tags.contains(t)) {
                    return false;
                }
                if !filter.statuses.is_empty() && !filter.statuses.contains(&l.irl_status) {
                    return false;
                }
                if !filter.districts.is_empty()
                    && !filter
                        .districts
                        .iter()
                        .any(|d| d == l.district.as_deref().unwrap_or(""))
                {
                    return false;
                }
                if !terms.is_empty() {
                    let hay = fold_text(&format!(
                        "{}\n{}\n{}\n{}",
                        l.id,
                        l.ig_address,
                        l.irl_address,
                        l.tags.join(" ")
                    ));
                    if !terms.iter().all(|t| hay.contains(t)) {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    fn nearest(&self, p: WorldPoint, n: usize) -> Vec<NearestLandmark> {
        let mut items: Vec<(usize, f64)> = self
            .landmarks
            .iter()
            .enumerate()
            .filter_map(|(i, l)| {
                let ig = l.ig?;
                Some((i, (ig.x - p.x).hypot(ig.y - p.y)))
            })
            .collect();
        items.sort_by(|a, b| a.1.total_cmp(&b.1));
        items.truncate(n);
        items
            .into_iter()
            .map(|(i, distance)| NearestLandmark {
                landmark: self.landmarks[i].clone(),
                distance,
            })
            .collect()
    }

    fn stats(&self) -> &LandmarkStats {
        &self.stats
    }

    fn geojson(&self, filter: &LandmarkFilter) -> Value {
        let features: Vec<Value> = self
            .filter(filter)
            .into_iter()
            .filter_map(|l| {
                let ig = l.ig?;
                Some(json!({
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

        json!({
            "type": "FeatureCollection",
            "crs": {
                "type": "name",
                "properties": { "name": "urn:leonida:world-metres" }
            },
            "features": features,
        })
    }

    fn predict_ig(&self, irl: LatLon, k: usize) -> Option<Prediction> {
        let model = self.irl_model.as_ref()?;
        predict_ig_local(model, irl, k)
    }

    fn triangulate(&self, rays: &[Ray2D]) -> Option<Triangulation> {
        triangulate_rays(rays)
    }
}

#[derive(Deserialize)]
struct Gta6MapTuple(
    String,
    String,
    Vec<f64>,
    i64,
    String,
    Vec<f64>,
    i64,
    Vec<String>,
    String,
    i64,
);

fn parse_gta6map_tuple(row: Gta6MapTuple, source: &str) -> Landmark {
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

    let ig = (ig_c.len() >= 2).then(|| WorldPoint {
        x: ig_c[0],
        y: ig_c[1],
    });
    let irl = (irl_c.len() >= 2).then(|| LatLon {
        lat: irl_c[0],
        lon: irl_c[1],
    });

    let norm_color = if color.len() == 6 && color.chars().all(|c| c.is_ascii_hexdigit()) {
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

fn split_address(ig_address: &str) -> (Option<String>, Option<String>) {
    let Some(idx) = ig_address.rfind(',') else {
        let trimmed = ig_address.trim();
        let b = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
        return (b, None);
    };
    let building = ig_address[..idx].trim();
    let district = ig_address[idx + 1..].trim();
    let b = if building.is_empty() || building == "?" {
        None
    } else {
        Some(building.to_string())
    };
    let d = if district.is_empty() {
        None
    } else {
        Some(district.to_string())
    };
    (b, d)
}

fn irl_name(irl_address: &str) -> String {
    irl_address
        .split(',')
        .next()
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| irl_address.to_string())
}

fn id_sort_key(id: &str) -> String {
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

fn fold_text(s: &str) -> String {
    s.to_lowercase()
}

fn compute_stats(list: &[Landmark]) -> LandmarkStats {
    let mut positioned = 0;
    let mut by_status = StatusCounts {
        confirmed: 0,
        unconfirmed: 0,
        unknown: 0,
    };
    let mut by_tag: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_district: BTreeMap<String, usize> = BTreeMap::new();

    for l in list {
        if l.ig.is_some() {
            positioned += 1;
        }
        match l.irl_status {
            IrlStatus::Confirmed => by_status.confirmed += 1,
            IrlStatus::Unconfirmed => by_status.unconfirmed += 1,
            IrlStatus::Unknown => by_status.unknown += 1,
        }
        for t in &l.tags {
            *by_tag.entry(t.clone()).or_insert(0) += 1;
        }
        let d = l.district.as_deref().unwrap_or("(none)").to_string();
        *by_district.entry(d).or_insert(0) += 1;
    }

    LandmarkStats {
        total: list.len(),
        positioned,
        by_status,
        by_tag,
        by_district,
    }
}

fn project_local(p: LatLon, origin: LatLon) -> WorldPoint {
    let r = PI / 180.0;
    WorldPoint {
        x: (p.lon - origin.lon) * r * EARTH_R * (origin.lat * r).cos(),
        y: (p.lat - origin.lat) * r * EARTH_R,
    }
}

fn build_irl_to_ig_model(anchors: &[(LatLon, WorldPoint)]) -> Option<IrlToIgModel> {
    if anchors.is_empty() {
        return None;
    }
    let n = anchors.len() as f64;
    let origin = LatLon {
        lat: anchors.iter().map(|(a, _)| a.lat).sum::<f64>() / n,
        lon: anchors.iter().map(|(a, _)| a.lon).sum::<f64>() / n,
    };
    let pairs = anchors
        .iter()
        .map(|(irl, ig)| PointPair {
            from: project_local(*irl, origin),
            to: *ig,
            weight: 1.0,
        })
        .collect();
    Some(IrlToIgModel { origin, pairs })
}

#[derive(Debug, Clone, Copy)]
struct Affine2D {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Affine2D {
    fn apply(self, p: WorldPoint) -> WorldPoint {
        WorldPoint {
            x: self.a * p.x + self.b * p.y + self.c,
            y: self.d * p.x + self.e * p.y + self.f,
        }
    }
}

fn det3(m: [[f64; 3]; 3]) -> f64 {
    m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
}

fn solve3(m: [[f64; 3]; 3], v: [f64; 3]) -> Option<[f64; 3]> {
    let d = det3(m);
    if d.abs() < 1e-12 {
        return None;
    }
    let mut out = [0.0; 3];
    for c in 0..3 {
        let mut mc = m;
        for i in 0..3 {
            mc[i][c] = v[i];
        }
        out[c] = det3(mc) / d;
    }
    Some(out)
}

fn fit_affine(pairs: &[PointPair]) -> Option<Affine2D> {
    if pairs.len() < 3 {
        return None;
    }
    let mut n_mat = [[0.0; 3]; 3];
    let mut bx = [0.0; 3];
    let mut by = [0.0; 3];
    for p in pairs {
        let row = [p.from.x, p.from.y, 1.0];
        for i in 0..3 {
            for j in 0..3 {
                n_mat[i][j] += p.weight * row[i] * row[j];
            }
            bx[i] += p.weight * row[i] * p.to.x;
            by[i] += p.weight * row[i] * p.to.y;
        }
    }
    let sx = solve3(n_mat, bx)?;
    let sy = solve3(n_mat, by)?;
    Some(Affine2D {
        a: sx[0],
        b: sx[1],
        c: sx[2],
        d: sy[0],
        e: sy[1],
        f: sy[2],
    })
}

fn predict_ig_local(model: &IrlToIgModel, irl: LatLon, k: usize) -> Option<Prediction> {
    if model.pairs.len() < 3 {
        return None;
    }
    let q = project_local(irl, model.origin);
    let mut ranked: Vec<(&PointPair, f64)> = model
        .pairs
        .iter()
        .map(|p| (p, (p.from.x - q.x).hypot(p.from.y - q.y)))
        .collect();
    ranked.sort_by(|a, b| a.1.total_cmp(&b.1));
    ranked.truncate(k.max(3));

    let scale = ranked
        .last()
        .map(|r| r.1)
        .filter(|&d| d != 0.0)
        .unwrap_or(1.0);
    let weighted: Vec<PointPair> = ranked
        .into_iter()
        .map(|(p, d)| PointPair {
            from: p.from,
            to: p.to,
            weight: 1.0 / (0.05 + (d / scale).powi(2)),
        })
        .collect();

    let t = fit_affine(&weighted)?;
    let se: f64 = weighted
        .iter()
        .map(|w| {
            let r = t.apply(w.from);
            (r.x - w.to.x).powi(2) + (r.y - w.to.y).powi(2)
        })
        .sum();

    Some(Prediction {
        point: t.apply(q),
        rms: (se / weighted.len() as f64).sqrt(),
        neighbours: weighted.len(),
    })
}

fn bearing_to_vector(bearing_deg: f64) -> WorldPoint {
    let r = bearing_deg.to_radians();
    WorldPoint {
        x: r.sin(),
        y: r.cos(),
    }
}

fn triangulate_rays(rays: &[Ray2D]) -> Option<Triangulation> {
    if rays.len() < 2 {
        return None;
    }
    let (mut a11, mut a12, mut a22, mut b1, mut b2) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for r in rays {
        let d = bearing_to_vector(r.bearing);
        let w = r.weight.unwrap_or(1.0);
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
