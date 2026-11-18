//! Leonida world coordinate frames.
//!
//! WORLD frame (canonical for this toolkit):
//!   - units: metres (community photogrammetry calibration, NOT an official Rockstar measurement)
//!   - +x = east, +y = north, +z = up (height above sea level)
//!   - origin and scale are shared with gtamaplib / gtamaplib-vc / gta6map.github.io (all public domain),
//!     so their datasets can be overlaid without re-projection.

use serde::{Deserialize, Serialize};

/// 2D point in Leonida world metres (`+x` = east, `+y` = north).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WorldPoint {
    pub x: f64,
    pub y: f64,
}

impl WorldPoint {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// 3D point in Leonida world metres (`+z` = height above sea level).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WorldPoint3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl WorldPoint3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

/// Geographic coordinates in WGS84 degrees.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

impl LatLon {
    pub const fn new(lat: f64, lon: f64) -> Self {
        Self { lat, lon }
    }
}

/// Bounds of the square region covered by the gta6map tile pyramid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldExtent {
    pub size: f64,
    pub half: f64,
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
}

/// The square region covered by the gta6map tile pyramid (32 768 m wide, centred on 0,0).
pub const WORLD_EXTENT: WorldExtent = WorldExtent {
    size: 32768.0,
    half: 16384.0,
    min_x: -16384.0,
    max_x: 16384.0,
    min_y: -16384.0,
    max_y: 16384.0,
};

/// Euclidean distance in 2D world metres.
pub fn distance(a: WorldPoint, b: WorldPoint) -> f64 {
    (b.x - a.x).hypot(b.y - a.y)
}

/// Euclidean distance in 3D world metres.
pub fn distance3(a: WorldPoint3, b: WorldPoint3) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let dz = b.z - a.z;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Compass bearing in degrees (`0` = north, `90` = east) from `a` to `b`.
pub fn bearing(a: WorldPoint, b: WorldPoint) -> f64 {
    let deg = (b.x - a.x).atan2(b.y - a.y).to_degrees();
    let wrapped = deg.rem_euclid(360.0);
    if wrapped >= 360.0 {
        0.0
    } else {
        wrapped
    }
}

/// Unit direction vector for a compass bearing in degrees.
pub fn bearing_to_vector(bearing_deg: f64) -> WorldPoint {
    let r = bearing_deg.to_radians();
    WorldPoint {
        x: r.sin(),
        y: r.cos(),
    }
}

/// Total polyline length in world metres.
pub fn path_length(points: &[WorldPoint]) -> f64 {
    points.windows(2).map(|w| distance(w[0], w[1])).sum()
}

/// Polygon area (m²) via the shoelace formula; vertices in either winding.
pub fn polygon_area(points: &[WorldPoint]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let mut s = 0.0;
    for (i, a) in points.iter().enumerate() {
        let b = points.get(i + 1).unwrap_or(&points[0]);
        s += a.x * b.y - b.x * a.y;
    }
    s.abs() / 2.0
}

/// Format a distance in metres as `"123 m"` or `"1.23 km"`.
pub fn format_distance(m: f64) -> String {
    if m >= 1000.0 {
        format!("{:.2} km", m / 1000.0)
    } else {
        format!("{m:.0} m")
    }
}

/// Format a `WorldPoint` with the requested decimal precision.
pub fn format_world(p: WorldPoint, digits: usize) -> String {
    format!("{:.digits$}, {:.digits$}", p.x, p.y)
}

/// Canonical bounds for the `gta6-leonida-atlas` presentation frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanonicalBounds {
    pub west: f64,
    pub east: f64,
    pub south: f64,
    pub north: f64,
}

/// Canonical `(east, north)` point in GTADB units.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CanonicalPoint {
    pub east: f64,
    pub north: f64,
}

/// SVG / three.js XZ-plane point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SvgPoint {
    pub x: f64,
    pub z: f64,
}

/// Presentation frame used by `gta6-leonida-atlas` (SVG / three.js XZ plane).
/// The atlas "canonical" unit is a GTADB unit; `svg = canonical * 2` with north flipped.
pub struct AtlasFrame;

impl AtlasFrame {
    pub const CANONICAL_BOUNDS: CanonicalBounds = CanonicalBounds {
        west: -16000.0,
        east: 4000.0,
        south: -8000.0,
        north: 12000.0,
    };

    pub fn canonical_to_svg(p: CanonicalPoint) -> SvgPoint {
        SvgPoint {
            x: p.east * 2.0,
            z: p.north * -2.0,
        }
    }

    pub fn svg_to_canonical(p: SvgPoint) -> CanonicalPoint {
        CanonicalPoint {
            east: p.x / 2.0,
            north: p.z / -2.0,
        }
    }
}

/// Convert Leonida world `(x, y)` metres to Atlas SVG/3D XZ `(sx, sz)` coordinates.
pub fn world_to_svg(x: f64, y: f64) -> (f64, f64) {
    (x * 2.0, -y * 2.0)
}

/// Convert Atlas SVG/3D XZ `(sx, sz)` coordinates back to Leonida world `(x, y)` metres.
pub fn svg_to_world(sx: f64, sz: f64) -> (f64, f64) {
    (sx / 2.0, -sz / 2.0)
}
