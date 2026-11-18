//! Leonida Toolkit core library (owner: core-rust).
//!
//! 1:1 Rust port of `@leonida/core` (`packages/core/src/*.ts`).

#![forbid(unsafe_code)]

pub mod companion;
pub mod coords;
pub mod error;
pub mod geo;
pub mod landmarks;
pub mod natives;
pub mod phone;
pub mod photogrammetry;
pub mod rage;
pub mod routing;
pub mod search;
pub mod sol;
pub mod tiles;

pub use companion::{companion_stats, filter_collectibles, CollectibleItem, CompanionStats};

pub use coords::{
    bearing, bearing_to_vector, distance, distance3, format_distance, format_world, path_length,
    polygon_area, svg_to_world, world_to_svg, AtlasFrame, CanonicalBounds, CanonicalPoint, LatLon,
    SvgPoint, WorldExtent, WorldPoint, WorldPoint3, WORLD_EXTENT,
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
pub use natives::{
    filter_natives, generate_shv_stub, native_stats, NativeEntry, NativeNamespaceStat, NativeParam,
};
pub use phone::{
    default_phone_contacts, filter_photos_by_district, messages_for_contact, phone_stats,
    unread_count, PhoneContact, PhoneMessage, PhoneNote, PhoneStats, SnapmaticPhoto,
};
pub use photogrammetry::{
    frustum_polygon, project_world_to_pixel, solve_camera_pose, CameraObs, CameraPose,
};
pub use rage::{
    compute_ng_key_index, compute_segment_size, export_handling_meta, joaat, parse_handling_meta,
    parse_rage_header, parse_rpf_header, parse_rsc7_header, HandlingItem, RageHeader,
    RpfEncryption, RpfHeader, RpfVersion, Rsc7Header, RPF7_MAGIC, RPF8_MAGIC, RSC7_MAGIC,
};
pub use routing::{plan_route, plan_route_from_landmarks, RouteResult};
pub use search::{
    filter_landmarks, nearest_landmarks, to_geojson, GeoJsonCrs, GeoJsonCrsProperties,
    GeoJsonFeature, GeoJsonFeatureCollection, GeoJsonLandmarkProperties, GeoJsonPointGeometry,
    LandmarkFilter, NearestLandmark,
};
pub use sol::{
    filter_sol_entries, group_sol_entries_by_dataset, parse_sol_entries,
    score_screenshot_challenge, sol_dataset_stats, ChallengeGrade, ChallengeScoreResult,
    ChangelogEntry, NewsItem, NotificationItem, RankInfo, SiteSettings, SolDatasetStats, SolEntry,
    SolEntryFilter,
};
pub use tiles::{
    metres_per_pixel, pixel_to_world, tile_bounds, tile_file_name, tile_for_world, tiles_per_side,
    world_to_pixel, PixelCoord, TileBounds, TileCoord, TileManifest, TileResolver, TileSource,
    MAX_TILE_Z, MIN_TILE_Z, TILE_SIZE,
};
