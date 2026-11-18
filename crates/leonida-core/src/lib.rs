//! Leonida Toolkit core library (owner: core-rust).
//!
//! 1:1 Rust port of `@leonida/core` (`packages/core/src/*.ts`).

#![forbid(unsafe_code)]

pub mod coords;
pub mod error;
pub mod geo;
pub mod landmarks;
pub mod search;
pub mod tiles;

pub use coords::{
    bearing, bearing_to_vector, distance, distance3, format_distance, format_world, path_length,
    polygon_area, AtlasFrame, CanonicalBounds, CanonicalPoint, LatLon, SvgPoint, WorldExtent,
    WorldPoint, WorldPoint3, WORLD_EXTENT,
};
pub use error::CoreError;
pub use geo::{
    apply_affine, build_irl_to_ig_model, fit_affine, haversine, intersect_rays, predict_ig,
    project_local, triangulate, Affine2D, Anchor, IrlToIgModel, PointPair, Prediction, Ray2D,
    Triangulation,
};
pub use landmarks::{
    id_sort_key, landmark_stats, parse_gta6map_tuple, parse_gtamaplib_vc, Gta6MapTuple,
    GtamaplibVcEntry, GtamaplibVcFile, IrlStatus, Landmark, Landmark3D, LandmarkStats,
    StatusCounts,
};
pub use search::{
    filter_landmarks, nearest_landmarks, to_geojson, GeoJsonCrs, GeoJsonCrsProperties,
    GeoJsonFeature, GeoJsonFeatureCollection, GeoJsonLandmarkProperties, GeoJsonPointGeometry,
    LandmarkFilter, NearestLandmark,
};
pub use tiles::{
    metres_per_pixel, pixel_to_world, tile_bounds, tile_file_name, tile_for_world, tiles_per_side,
    world_to_pixel, PixelCoord, TileBounds, TileCoord, TileManifest, TileResolver, TileSource,
    MAX_TILE_Z, MIN_TILE_Z, TILE_SIZE,
};
