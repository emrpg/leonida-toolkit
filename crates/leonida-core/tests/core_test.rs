use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use leonida_core::{
    apply_affine, bearing, build_irl_to_ig_model, distance, distance3, filter_landmarks,
    fit_affine, format_distance, format_world, haversine, id_sort_key, intersect_rays,
    landmark_stats, metres_per_pixel, nearest_landmarks, parse_gta6map_tuple, parse_gtamaplib_vc,
    path_length, pixel_to_world, polygon_area, predict_ig, tile_bounds, tile_file_name,
    tile_for_world, tiles_per_side, to_geojson, triangulate, world_to_pixel, Affine2D, Anchor,
    AtlasFrame, CanonicalPoint, CoreError, Gta6MapTuple, GtamaplibVcFile, IrlStatus, Landmark,
    LandmarkFilter, LatLon, PixelCoord, PointPair, Ray2D, SvgPoint, TileCoord, TileManifest,
    TileResolver, TileSource, WorldPoint, WorldPoint3,
};

fn raw_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../data/raw")
        .join(name)
}

fn load_gta6map_landmarks() -> Vec<Landmark> {
    let raw = fs::read_to_string(raw_path("gta6map-landmarks.json"))
        .expect("read gta6map-landmarks.json");
    let rows: Vec<Gta6MapTuple> = serde_json::from_str(&raw).expect("parse gta6map-landmarks.json");
    rows.iter()
        .map(|r| parse_gta6map_tuple(r, "gta6map.github.io"))
        .collect()
}

fn assert_close(actual: f64, expected: f64, tol: f64) {
    let diff = (actual - expected).abs();
    assert!(
        diff <= tol,
        "expected {expected} (±{tol}), got {actual} (diff {diff})"
    );
}

#[test]
fn tiles_pyramid_scale() {
    assert_eq!(metres_per_pixel(0), 32.0);
    assert_eq!(metres_per_pixel(5), 1.0);
    assert_eq!(metres_per_pixel(6), 0.5);
    assert_eq!(tiles_per_side(0), 1);
    assert_eq!(tiles_per_side(6), 64);
}

#[test]
fn tiles_round_trip_world_pixel() {
    let p = WorldPoint::new(1340.0, 2830.0);
    let px = world_to_pixel(p, 6);
    assert_eq!(pixel_to_world(px.px, px.py, 6), p);
    assert_eq!(
        world_to_pixel(WorldPoint::new(-16384.0, 16384.0), 3),
        PixelCoord { px: 0.0, py: 0.0 }
    );
}

#[test]
fn tiles_find_right_tile_and_bounds() {
    assert_eq!(
        tile_for_world(WorldPoint::new(0.0, 0.0), 1),
        TileCoord::new(1, 1, 1)
    );
    assert_eq!(
        tile_for_world(WorldPoint::new(-1.0, 1.0), 1),
        TileCoord::new(1, 0, 0)
    );

    let b = tile_bounds(TileCoord::new(0, 0, 0));
    assert_eq!(b.min, WorldPoint::new(-16384.0, -16384.0));
    assert_eq!(b.max, WorldPoint::new(16384.0, 16384.0));
    assert_eq!(
        tile_file_name(TileCoord::new(2, 3, 1), "jpg"),
        "2/2,1,3.jpg"
    );
}

#[test]
fn tiles_resolves_composite_before_base() {
    let mut manifest: TileManifest = BTreeMap::new();
    manifest.insert(
        "base".to_string(),
        BTreeMap::from([("2".to_string(), vec!["1,1".to_string(), "1,2".to_string()])]),
    );
    manifest.insert(
        "comp".to_string(),
        BTreeMap::from([("2".to_string(), vec!["1,2".to_string()])]),
    );
    let r = TileResolver::new(&manifest);

    let with_comp = TileSource {
        base: "base".to_string(),
        composite: Some("comp".to_string()),
    };
    let base_only = TileSource {
        base: "base".to_string(),
        composite: None,
    };

    assert_eq!(r.resolve(&with_comp, TileCoord::new(2, 2, 1)), Some("comp"));
    assert_eq!(r.resolve(&with_comp, TileCoord::new(2, 1, 1)), Some("base"));
    assert_eq!(r.resolve(&base_only, TileCoord::new(2, 0, 0)), None);
    assert_eq!(r.tilesets(), vec!["base", "comp"]);
}

#[test]
fn geometry_bearing_is_compass_based() {
    assert_close(
        bearing(WorldPoint::new(0.0, 0.0), WorldPoint::new(0.0, 10.0)),
        0.0,
        1e-9,
    );
    assert_close(
        bearing(WorldPoint::new(0.0, 0.0), WorldPoint::new(10.0, 0.0)),
        90.0,
        1e-9,
    );
    assert_close(
        bearing(WorldPoint::new(0.0, 0.0), WorldPoint::new(-10.0, 0.0)),
        270.0,
        1e-9,
    );
}

#[test]
fn geometry_polygon_area_and_path() {
    let rect = [
        WorldPoint::new(0.0, 0.0),
        WorldPoint::new(10.0, 0.0),
        WorldPoint::new(10.0, 5.0),
        WorldPoint::new(0.0, 5.0),
    ];
    assert_eq!(polygon_area(&rect), 50.0);
    assert_eq!(polygon_area(&rect[..2]), 0.0);
    assert_eq!(path_length(&rect), 25.0);
    assert_eq!(
        distance(WorldPoint::new(0.0, 0.0), WorldPoint::new(3.0, 4.0)),
        5.0
    );
    assert_eq!(
        distance3(
            WorldPoint3::new(0.0, 0.0, 0.0),
            WorldPoint3::new(2.0, 3.0, 6.0)
        ),
        7.0
    );
    assert_eq!(format_distance(420.4), "420 m");
    assert_eq!(format_distance(1500.0), "1.50 km");
    assert_eq!(format_world(WorldPoint::new(1.25, -3.5), 1), "1.2, -3.5");
}

#[test]
fn coords_atlas_frame_round_trip() {
    let c = CanonicalPoint {
        east: 1200.0,
        north: -350.0,
    };
    let s = AtlasFrame::canonical_to_svg(c);
    assert_eq!(
        s,
        SvgPoint {
            x: 2400.0,
            z: 700.0
        }
    );
    assert_eq!(AtlasFrame::svg_to_canonical(s), c);
}

#[test]
fn geometry_intersects_two_rays() {
    let p = intersect_rays(
        Ray2D::new(WorldPoint::new(0.0, 0.0), 45.0),
        Ray2D::new(WorldPoint::new(10.0, 0.0), 315.0),
    )
    .expect("rays should intersect");
    assert_close(p.x, 5.0, 1e-9);
    assert_close(p.y, 5.0, 1e-9);

    assert_eq!(
        intersect_rays(
            Ray2D::new(WorldPoint::new(0.0, 0.0), 0.0),
            Ray2D::new(WorldPoint::new(1.0, 0.0), 0.0),
        ),
        None
    );
    // Intersection behind ray 1 origin
    assert_eq!(
        intersect_rays(
            Ray2D::new(WorldPoint::new(0.0, 0.0), 225.0),
            Ray2D::new(WorldPoint::new(10.0, 0.0), 315.0),
        ),
        None
    );
}

#[test]
fn geometry_triangulates_n_rays() {
    let target = WorldPoint::new(300.0, -120.0);
    let origins = [
        WorldPoint::new(0.0, 0.0),
        WorldPoint::new(1000.0, 50.0),
        WorldPoint::new(200.0, -900.0),
    ];
    let rays: Vec<Ray2D> = origins
        .iter()
        .map(|&o| Ray2D::new(o, bearing(o, target)))
        .collect();
    let res = triangulate(&rays).expect("triangulate should succeed");
    assert_close(res.point.x, target.x, 1e-6);
    assert_close(res.point.y, target.y, 1e-6);
    assert!(res.rms < 1e-6);

    assert_eq!(triangulate(&rays[..1]), None);
}

#[test]
fn geometry_fits_exact_affine() {
    let t = Affine2D {
        a: 0.5,
        b: -0.2,
        c: 100.0,
        d: 0.1,
        e: 0.7,
        f: -50.0,
    };
    let pts = [
        WorldPoint::new(0.0, 0.0),
        WorldPoint::new(10.0, 3.0),
        WorldPoint::new(-4.0, 8.0),
        WorldPoint::new(7.0, -6.0),
    ];
    let pairs: Vec<PointPair> = pts
        .iter()
        .map(|&p| PointPair::new(p, apply_affine(t, p)))
        .collect();
    let fit = fit_affine(&pairs).expect("fit_affine should succeed");
    assert_close(fit.a, 0.5, 1e-9);
    assert_close(fit.b, -0.2, 1e-9);
    assert_close(fit.c, 100.0, 1e-9);
    assert_close(fit.d, 0.1, 1e-9);
    assert_close(fit.e, 0.7, 1e-9);
    assert_close(fit.f, -50.0, 1e-9);

    assert_eq!(fit_affine(&pairs[..2]), None);
}

#[test]
fn geo_haversine_and_empty_anchors() {
    let a = LatLon::new(25.7617, -80.1918);
    let b = LatLon::new(25.7617, -80.1918);
    assert_eq!(haversine(a, b), 0.0);
    let miami_to_bal_harbour = haversine(a, LatLon::new(25.8990, -80.1233));
    assert!(miami_to_bal_harbour > 15_000.0 && miami_to_bal_harbour < 18_000.0);

    assert!(matches!(
        build_irl_to_ig_model(&[]),
        Err(CoreError::NoAnchors)
    ));
}

#[test]
fn real_datasets_parse_gta6map_landmarks() {
    let landmarks = load_gta6map_landmarks();
    assert!(landmarks.len() > 700);
    let b1 = landmarks
        .iter()
        .find(|l| l.id == "b1")
        .expect("b1 should exist");
    assert_eq!(b1.district.as_deref(), Some("Vice Beach"));
    assert_eq!(b1.ig, Some(WorldPoint::new(1340.0, 2830.0)));
    assert_eq!(b1.irl_status, IrlStatus::Confirmed);
    assert!(b1.name.contains("Ritz-Carlton"));

    let stats = landmark_stats(&landmarks);
    assert_eq!(stats.total, landmarks.len());
    assert!(stats.positioned > 500);
    assert_eq!(id_sort_key("b12"), "b000012");
    assert_eq!(id_sort_key("L264"), "L000264");
}

#[test]
fn real_datasets_filter_nearest_and_geojson() {
    let landmarks = load_gta6map_landmarks();
    let hotels = filter_landmarks(
        &landmarks,
        &LandmarkFilter {
            tags: Some(vec!["hotel".to_string()]),
            positioned_only: true,
            ..Default::default()
        },
    );
    assert!(hotels.len() > 10);

    let ritz = filter_landmarks(
        &landmarks,
        &LandmarkFilter {
            text: Some("ritz carlton".to_string()),
            ..Default::default()
        },
    );
    assert!(ritz.iter().any(|l| l.id == "b1"));

    let nearest = nearest_landmarks(&landmarks, WorldPoint::new(1340.0, 2830.0), 1);
    assert_eq!(nearest.len(), 1);
    assert_eq!(nearest[0].landmark.id, "b1");

    let geojson = to_geojson(&landmarks);
    assert_eq!(geojson.collection_type, "FeatureCollection");
    assert_eq!(geojson.crs.properties.name, "urn:leonida:world-metres");
    assert!(!geojson.features.is_empty());
}

#[test]
fn real_datasets_parse_gtamaplib_vc_3d_landmarks() {
    let raw = fs::read_to_string(raw_path("gtamaplib-vc-landmarks.json"))
        .expect("read gtamaplib-vc-landmarks.json");
    let doc: GtamaplibVcFile = serde_json::from_str(&raw).expect("parse gtamaplib-vc JSON");
    let l3 = parse_gtamaplib_vc(&doc, "gtamaplib-vc").expect("parse gtamaplib-vc landmarks");
    assert!(l3.len() > 50);
    assert!(l3.iter().all(|l| l.xyz.z.is_finite()));

    let bad = GtamaplibVcFile {
        schema: "unknown-v0".to_string(),
        landmarks: vec![],
    };
    assert!(matches!(
        parse_gtamaplib_vc(&bad, "gtamaplib-vc"),
        Err(CoreError::UnsupportedSchema(_))
    ));
}

#[test]
fn real_datasets_predict_ig_leave_one_out() {
    let landmarks = load_gta6map_landmarks();
    let anchors: Vec<Anchor> = landmarks
        .iter()
        .filter_map(|l| {
            if l.irl_status == IrlStatus::Confirmed {
                Some(Anchor {
                    irl: l.irl?,
                    ig: l.ig?,
                })
            } else {
                None
            }
        })
        .collect();
    assert!(anchors.len() > 100);

    let mut errors = Vec::with_capacity(anchors.len());
    for i in 0..anchors.len() {
        let mut rest = Vec::with_capacity(anchors.len() - 1);
        rest.extend_from_slice(&anchors[..i]);
        rest.extend_from_slice(&anchors[i + 1..]);
        let model = build_irl_to_ig_model(&rest).expect("build model");
        if let Some(pred) = predict_ig(&model, anchors[i].irl, 10) {
            let err = (pred.point.x - anchors[i].ig.x).hypot(pred.point.y - anchors[i].ig.y);
            errors.push(err);
        }
    }

    errors.sort_by(f64::total_cmp);
    let median = errors[errors.len() / 2];
    let p90 = errors[(errors.len() * 9) / 10];
    println!(
        "IRL->IG leave-one-out: n={} median={median:.0}m p90={p90:.0}m",
        errors.len()
    );
    assert!(median < 400.0, "median error {median:.1}m >= 400m");
}
