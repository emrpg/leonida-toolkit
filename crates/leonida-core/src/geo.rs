//! Geodesy, local affine fitting, IRL->IG prediction, and bearing triangulation.

use serde::{Deserialize, Serialize};

use crate::coords::{bearing_to_vector, LatLon, WorldPoint};
use crate::error::CoreError;

const EARTH_R: f64 = 6_371_008.8;

/// Great-circle distance in metres.
pub fn haversine(a: LatLon, b: LatLon) -> f64 {
    let d_lat = (b.lat - a.lat).to_radians();
    let d_lon = (b.lon - a.lon).to_radians();
    let sin_dlat = (d_lat / 2.0).sin();
    let sin_dlon = (d_lon / 2.0).sin();
    let h = sin_dlat * sin_dlat
        + a.lat.to_radians().cos() * b.lat.to_radians().cos() * sin_dlon * sin_dlon;
    2.0 * EARTH_R * h.clamp(0.0, 1.0).sqrt().asin()
}

/// Local equirectangular projection (metres, `x` east / `y` north) around an origin.
pub fn project_local(p: LatLon, origin: LatLon) -> WorldPoint {
    WorldPoint {
        x: (p.lon - origin.lon).to_radians() * EARTH_R * origin.lat.to_radians().cos(),
        y: (p.lat - origin.lat).to_radians() * EARTH_R,
    }
}

/// 2D affine transform: `x' = a*x + b*y + c`, `y' = d*x + e*y + f`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Affine2D {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

pub fn apply_affine(t: Affine2D, p: WorldPoint) -> WorldPoint {
    WorldPoint {
        x: t.a * p.x + t.b * p.y + t.c,
        y: t.d * p.x + t.e * p.y + t.f,
    }
}

fn det3(q: [[f64; 3]; 3]) -> f64 {
    q[0][0] * (q[1][1] * q[2][2] - q[1][2] * q[2][1])
        - q[0][1] * (q[1][0] * q[2][2] - q[1][2] * q[2][0])
        + q[0][2] * (q[1][0] * q[2][1] - q[1][1] * q[2][0])
}

/// Solve a 3x3 linear system (Cramer's rule). Returns `None` when singular.
fn solve3(m: [[f64; 3]; 3], v: [f64; 3]) -> Option<[f64; 3]> {
    let d = det3(m);
    if d.abs() < 1e-12 {
        return None;
    }
    let mut out = [0.0; 3];
    for c in 0..3 {
        let mut mc = m;
        for (row, &rhs) in mc.iter_mut().zip(v.iter()) {
            row[c] = rhs;
        }
        out[c] = det3(mc) / d;
    }
    Some(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PointPair {
    pub from: WorldPoint,
    pub to: WorldPoint,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
}

impl PointPair {
    pub const fn new(from: WorldPoint, to: WorldPoint) -> Self {
        Self {
            from,
            to,
            weight: None,
        }
    }

    pub const fn weighted(from: WorldPoint, to: WorldPoint, weight: f64) -> Self {
        Self {
            from,
            to,
            weight: Some(weight),
        }
    }
}

/// Weighted least-squares affine fit. Needs at least 3 non-collinear pairs.
pub fn fit_affine(pairs: &[PointPair]) -> Option<Affine2D> {
    if pairs.len() < 3 {
        return None;
    }
    let mut n = [[0.0_f64; 3]; 3];
    let mut bx = [0.0_f64; 3];
    let mut by = [0.0_f64; 3];

    for pair in pairs {
        let weight = pair.weight.unwrap_or(1.0);
        let row = [pair.from.x, pair.from.y, 1.0];
        for i in 0..3 {
            for j in 0..3 {
                n[i][j] += weight * row[i] * row[j];
            }
            bx[i] += weight * row[i] * pair.to.x;
            by[i] += weight * row[i] * pair.to.y;
        }
    }

    let sx = solve3(n, bx)?;
    let sy = solve3(n, by)?;
    Some(Affine2D {
        a: sx[0],
        b: sx[1],
        c: sx[2],
        d: sy[0],
        e: sy[1],
        f: sy[2],
    })
}

/// Anchor pair linking a confirmed real-world coordinate to an in-game coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    pub irl: LatLon,
    pub ig: WorldPoint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrlToIgModel {
    pub origin: LatLon,
    pub pairs: Vec<PointPair>,
}

/// Build an IRL->in-game model from landmarks that have both confirmed real-world and in-game positions.
pub fn build_irl_to_ig_model(anchors: &[Anchor]) -> Result<IrlToIgModel, CoreError> {
    if anchors.is_empty() {
        return Err(CoreError::NoAnchors);
    }
    let count = anchors.len() as f64;
    let origin = LatLon {
        lat: anchors.iter().map(|a| a.irl.lat).sum::<f64>() / count,
        lon: anchors.iter().map(|a| a.irl.lon).sum::<f64>() / count,
    };
    let pairs = anchors
        .iter()
        .map(|a| PointPair::new(project_local(a.irl, origin), a.ig))
        .collect();
    Ok(IrlToIgModel { origin, pairs })
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Prediction {
    pub point: WorldPoint,
    /// RMS residual (in-game metres) of the local fit over the neighbours used.
    pub rms: f64,
    pub neighbours: usize,
}

/// Predict the in-game position of a real-world location with a locally weighted affine fit
/// over the `k` nearest anchors (default `k = 10` in the TS reference).
pub fn predict_ig(model: &IrlToIgModel, irl: LatLon, k: usize) -> Option<Prediction> {
    if model.pairs.is_empty() {
        return None;
    }
    let q = project_local(irl, model.origin);
    let mut ranked: Vec<(PointPair, f64)> = model
        .pairs
        .iter()
        .map(|&p| (p, (p.from.x - q.x).hypot(p.from.y - q.y)))
        .collect();
    ranked.sort_by(|a, b| a.1.total_cmp(&b.1));
    ranked.truncate(k.max(3));

    let last_d = ranked.last()?.1;
    let scale = if last_d == 0.0 { 1.0 } else { last_d };

    let weighted: Vec<PointPair> = ranked
        .iter()
        .map(|&(p, d)| {
            let ratio = d / scale;
            PointPair {
                from: p.from,
                to: p.to,
                weight: Some(1.0 / (0.05 + ratio * ratio)),
            }
        })
        .collect();

    let t = fit_affine(&weighted)?;
    let mut se = 0.0;
    for w in &weighted {
        let r = apply_affine(t, w.from);
        let dx = r.x - w.to.x;
        let dy = r.y - w.to.y;
        se += dx * dx + dy * dy;
    }

    Some(Prediction {
        point: apply_affine(t, q),
        rms: (se / weighted.len() as f64).sqrt(),
        neighbours: weighted.len(),
    })
}

/// A ray on the 2D map: origin + compass bearing (degrees).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Ray2D {
    pub origin: WorldPoint,
    pub bearing: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
}

impl Ray2D {
    pub const fn new(origin: WorldPoint, bearing: f64) -> Self {
        Self {
            origin,
            bearing,
            weight: None,
        }
    }

    pub const fn weighted(origin: WorldPoint, bearing: f64, weight: f64) -> Self {
        Self {
            origin,
            bearing,
            weight: Some(weight),
        }
    }
}

/// Intersection of two rays; `None` if parallel or behind either origin.
pub fn intersect_rays(r1: Ray2D, r2: Ray2D) -> Option<WorldPoint> {
    let d1 = bearing_to_vector(r1.bearing);
    let d2 = bearing_to_vector(r2.bearing);
    let den = d1.x * d2.y - d1.y * d2.x;
    if den.abs() < 1e-9 {
        return None;
    }
    let dx = r2.origin.x - r1.origin.x;
    let dy = r2.origin.y - r1.origin.y;
    let t1 = (dx * d2.y - dy * d2.x) / den;
    let t2 = (dx * d1.y - dy * d1.x) / den;
    if t1 < 0.0 || t2 < 0.0 {
        return None;
    }
    Some(WorldPoint {
        x: r1.origin.x + t1 * d1.x,
        y: r1.origin.y + t1 * d1.y,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Triangulation {
    pub point: WorldPoint,
    pub rms: f64,
}

/// Least-squares triangulation from `N >= 2` bearing rays: minimises the sum of squared
/// perpendicular distances to every ray. Returns point + RMS perpendicular residual.
pub fn triangulate(rays: &[Ray2D]) -> Option<Triangulation> {
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
    let mut se = 0.0;
    for r in rays {
        let d = bearing_to_vector(r.bearing);
        let vx = point.x - r.origin.x;
        let vy = point.y - r.origin.y;
        let perp = vx * d.y - vy * d.x;
        se += perp * perp;
    }
    Some(Triangulation {
        point,
        rms: (se / rays.len() as f64).sqrt(),
    })
}
