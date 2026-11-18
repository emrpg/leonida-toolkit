use std::fs;
use std::path::PathBuf;

use leonida_core::{
    frustum_polygon, parse_gta6map_tuple, plan_route, plan_route_from_landmarks,
    project_world_to_pixel, solve_camera_pose, svg_to_world, world_to_svg, CameraObs, CameraPose,
    Gta6MapTuple, Landmark,
};

fn load_real_landmarks() -> Vec<Landmark> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/raw/gta6map-landmarks.json");
    let raw = fs::read_to_string(path).expect("read gta6map-landmarks.json");
    let rows: Vec<Gta6MapTuple> = serde_json::from_str(&raw).expect("parse landmarks");
    rows.iter()
        .map(|r| parse_gta6map_tuple(r, "gta6map.github.io"))
        .collect()
}

#[test]
fn v04_atlas_world_svg_round_trip() {
    assert_eq!(world_to_svg(100.0, -250.0), (200.0, 500.0));
    assert_eq!(svg_to_world(200.0, 500.0), (100.0, -250.0));

    let (sx, sz) = world_to_svg(1340.0, 2830.0);
    let (wx, wy) = svg_to_world(sx, sz);
    assert!((wx - 1340.0).abs() < 1e-12);
    assert!((wy - 2830.0).abs() < 1e-12);
}

#[test]
fn v04_photogrammetry_solve_camera_pose_and_frustum() {
    let true_pose = CameraPose {
        x: 100.0,
        y: -400.0,
        z: 45.0,
        yaw_deg: 25.0,
        pitch_deg: -6.0,
        fov_deg: 60.0,
        rmse_px: 0.0,
    };
    let width = 1920.0;
    let height = 1080.0;

    let world_pts = [
        [200.0, -50.0, 30.0],
        [400.0, 120.0, 80.0],
        [150.0, 180.0, 10.0],
        [320.0, 40.0, 120.0],
        [240.0, 220.0, 55.0],
        [460.0, -20.0, 40.0],
    ];

    let obs: Vec<CameraObs> = world_pts
        .iter()
        .map(|&w| {
            let (px, py) = project_world_to_pixel(&true_pose, w, width, height)
                .expect("point should be in front of camera");
            CameraObs {
                world_x: w[0],
                world_y: w[1],
                world_z: w[2],
                px,
                py,
            }
        })
        .collect();

    let solved = solve_camera_pose(&obs, width, height, Some(55.0)).expect("pose should solve");
    assert!(
        solved.rmse_px < 1e-2,
        "expected rmse_px < 0.01, got {}",
        solved.rmse_px
    );
    assert!((solved.x - true_pose.x).abs() < 1.0);
    assert!((solved.y - true_pose.y).abs() < 1.0);
    assert!((solved.z - true_pose.z).abs() < 1.0);

    // Invalid inputs return None without panic
    assert!(solve_camera_pose(&obs[..2], width, height, None).is_none());
    assert!(solve_camera_pose(&obs, 0.0, height, None).is_none());

    let poly = frustum_polygon(solved.x, solved.y, solved.yaw_deg, solved.fov_deg, 500.0);
    assert_eq!(poly.len(), 19);
    assert_eq!(poly.first(), poly.last());
    assert!(frustum_polygon(0.0, 0.0, 0.0, 60.0, -10.0).is_empty());
}

#[test]
fn v04_routing_knn_dijkstra_and_real_landmarks() {
    let nodes = [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [200.0, 100.0]];
    let route = plan_route(&nodes, [-10.0, 0.0], [210.0, 100.0], 2).expect("route should exist");
    assert!(route.waypoints.len() >= 3);
    assert_eq!(route.waypoints.first().copied(), Some([-10.0, 0.0]));
    assert_eq!(route.waypoints.last().copied(), Some([210.0, 100.0]));
    assert!(route.distance_m > 0.0);
    assert!(route.fly_min < route.drive_min);

    // Empty nodes falls back to direct route; non-finite returns None
    let direct = plan_route(&[], [0.0, 0.0], [300.0, 400.0], 4).expect("direct fallback");
    assert_eq!(direct.waypoints, vec![[0.0, 0.0], [300.0, 400.0]]);
    assert!((direct.distance_m - 500.0).abs() < 1e-9);
    assert!(plan_route(&nodes, [f64::NAN, 0.0], [0.0, 0.0], 4).is_none());

    // Real landmark graph routing
    let landmarks = load_real_landmarks();
    let real_route = plan_route_from_landmarks(&landmarks, [1340.0, 2830.0], [-2500.0, -2300.0], 6)
        .expect("real landmark route");
    assert!(real_route.waypoints.len() >= 3);
    assert!(real_route.distance_m >= 5000.0);
}
