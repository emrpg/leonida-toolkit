//! Tile pyramid compatible with gta6map.github.io `tiles.py`:
//!   - 1024 px tiles, levels 0..6, tile file `{z}/{z},{y},{x}.jpg`
//!   - level 0: whole 32 768 m square in one tile (32 m/px); level 5: 1 m/px; level 6: 0.5 m/px
//!   - tile (0,0) is the north-west corner at world (-16384, +16384)

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::coords::{WorldPoint, WORLD_EXTENT};

pub const TILE_SIZE: u32 = 1024;
pub const MIN_TILE_Z: u8 = 0;
pub const MAX_TILE_Z: u8 = 6;

/// Tile coordinate `(z, x, y)` in the gta6map pyramid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileCoord {
    pub z: u8,
    pub x: i32,
    pub y: i32,
}

impl TileCoord {
    pub const fn new(z: u8, x: i32, y: i32) -> Self {
        Self { z, x, y }
    }
}

/// Global pixel coordinates at a zoom level (origin = NW corner).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PixelCoord {
    pub px: f64,
    pub py: f64,
}

/// World-space bounding box of a tile.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TileBounds {
    pub min: WorldPoint,
    pub max: WorldPoint,
}

/// Number of tiles along each axis at zoom level `z`.
pub fn tiles_per_side(z: u8) -> u32 {
    1_u32.checked_shl(u32::from(z)).unwrap_or(u32::MAX)
}

/// Ground resolution in metres per pixel at zoom level `z`.
pub fn metres_per_pixel(z: u8) -> f64 {
    WORLD_EXTENT.size / (2.0_f64.powi(i32::from(z)) * f64::from(TILE_SIZE))
}

/// World metres -> global pixel coordinates at level `z` (origin = NW corner).
pub fn world_to_pixel(p: WorldPoint, z: u8) -> PixelCoord {
    let mpp = metres_per_pixel(z);
    PixelCoord {
        px: (p.x + WORLD_EXTENT.half) / mpp,
        py: (WORLD_EXTENT.half - p.y) / mpp,
    }
}

/// Global pixel coordinates at level `z` -> world metres.
pub fn pixel_to_world(px: f64, py: f64, z: u8) -> WorldPoint {
    let mpp = metres_per_pixel(z);
    WorldPoint {
        x: px * mpp - WORLD_EXTENT.half,
        y: WORLD_EXTENT.half - py * mpp,
    }
}

/// Tile coordinate containing world point `p` at zoom level `z`.
pub fn tile_for_world(p: WorldPoint, z: u8) -> TileCoord {
    let PixelCoord { px, py } = world_to_pixel(p, z);
    let tile_size = f64::from(TILE_SIZE);
    TileCoord {
        z,
        x: (px / tile_size).floor() as i32,
        y: (py / tile_size).floor() as i32,
    }
}

/// World-space bounds of a tile.
pub fn tile_bounds(t: TileCoord) -> TileBounds {
    let tile_size = f64::from(TILE_SIZE);
    let nw = pixel_to_world(f64::from(t.x) * tile_size, f64::from(t.y) * tile_size, t.z);
    let se = pixel_to_world(
        f64::from(t.x + 1) * tile_size,
        f64::from(t.y + 1) * tile_size,
        t.z,
    );
    TileBounds {
        min: WorldPoint::new(nw.x, se.y),
        max: WorldPoint::new(se.x, nw.y),
    }
}

/// Relative tile filename `{z}/{z},{y},{x}.{ext}`.
pub fn tile_file_name(t: TileCoord, ext: &str) -> String {
    format!("{}/{},{},{}.{}", t.z, t.z, t.y, t.x, ext)
}

/// Availability manifest: `tileset id -> level -> list of "y,x" keys`.
pub type TileManifest = BTreeMap<String, BTreeMap<String, Vec<String>>>;

/// Base + optional composited overlay tileset pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileSource {
    /// Base map tileset, e.g. `"dupzor,51"`.
    pub base: String,
    /// Optional composited overlay tileset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composite: Option<String>,
}

/// Resolves the tileset directory that holds a tile, preferring the composited overlay set
/// (which only stores tiles touched by overlays) and falling back to the base set.
#[derive(Debug, Clone, Default)]
pub struct TileResolver {
    index: HashMap<String, HashMap<u8, HashSet<(i32, i32)>>>,
    tileset_names: Vec<String>,
}

impl TileResolver {
    pub fn new(manifest: &TileManifest) -> Self {
        let mut index = HashMap::with_capacity(manifest.len());
        let mut tileset_names = Vec::with_capacity(manifest.len());

        for (set, levels) in manifest {
            let mut level_map = HashMap::with_capacity(levels.len());
            for (z_str, keys) in levels {
                let Ok(z) = z_str.parse::<u8>() else {
                    continue;
                };
                let mut coords = HashSet::with_capacity(keys.len());
                for key in keys {
                    if let Some((y_str, x_str)) = key.split_once(',') {
                        if let (Ok(y), Ok(x)) =
                            (y_str.trim().parse::<i32>(), x_str.trim().parse::<i32>())
                        {
                            coords.insert((y, x));
                        }
                    }
                }
                level_map.insert(z, coords);
            }
            tileset_names.push(set.clone());
            index.insert(set.clone(), level_map);
        }

        Self {
            index,
            tileset_names,
        }
    }

    pub fn has(&self, set: &str, t: TileCoord) -> bool {
        self.index
            .get(set)
            .and_then(|levels| levels.get(&t.z))
            .is_some_and(|coords| coords.contains(&(t.y, t.x)))
    }

    pub fn tilesets(&self) -> Vec<&str> {
        self.tileset_names.iter().map(String::as_str).collect()
    }

    pub fn resolve<'a>(&'a self, src: &'a TileSource, t: TileCoord) -> Option<&'a str> {
        if let Some(composite) = src.composite.as_deref() {
            if self.has(composite, t) {
                return Some(composite);
            }
        }
        if self.has(&src.base, t) {
            return Some(&src.base);
        }
        None
    }
}
