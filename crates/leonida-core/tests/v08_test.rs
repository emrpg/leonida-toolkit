use leonida_core::{
    compute_heatmap_cells, interpolate_motion_path, AdminDbOverview, MarkerComment, WikiCacheEntry,
};

#[test]
fn v08_heatmap_cells_aggregation_and_bounds() {
    let pts = [
        [10.0, 10.0],
        [50.0, 80.0],
        [120.0, 10.0],
        [-50.0, -50.0],
        [f64::NAN, 0.0],
    ];
    let cells = compute_heatmap_cells(&pts, 100.0);
    assert_eq!(cells.len(), 3);
    assert_eq!(cells[0].count, 2);
    assert!((cells[0].weight - 1.0).abs() < 1e-12);
    assert!((cells[0].x - 50.0).abs() < 1e-12);
    assert!((cells[0].y - 50.0).abs() < 1e-12);
    assert!((cells[1].weight - 0.5).abs() < 1e-12);

    assert!(compute_heatmap_cells(&[], 100.0).is_empty());
    assert!(compute_heatmap_cells(&pts, 0.0).is_empty());
    assert!(compute_heatmap_cells(&pts, -10.0).is_empty());
}

#[test]
fn v08_interpolate_motion_path_samples() {
    let path = [[0.0, 0.0], [0.0, 100.0], [100.0, 100.0]];

    let mid1 = interpolate_motion_path(&path, 0.25).expect("sample at 0.25");
    assert!((mid1.x - 0.0).abs() < 1e-9);
    assert!((mid1.y - 50.0).abs() < 1e-9);
    assert!((mid1.bearing_deg - 0.0).abs() < 1e-9);

    let mid2 = interpolate_motion_path(&path, 0.75).expect("sample at 0.75");
    assert!((mid2.x - 50.0).abs() < 1e-9);
    assert!((mid2.y - 100.0).abs() < 1e-9);
    assert!((mid2.bearing_deg - 90.0).abs() < 1e-9);

    let single = interpolate_motion_path(&[[12.0, -34.0]], 0.5).expect("single point");
    assert_eq!(single.x, 12.0);
    assert_eq!(single.y, -34.0);

    assert!(interpolate_motion_path(&[], 0.5).is_none());
    assert!(interpolate_motion_path(&path, f64::NAN).is_none());
}

#[test]
fn v08_models_serde_round_trip() {
    let comment = MarkerComment {
        id: "cmt-1".to_string(),
        marker_id: "b1".to_string(),
        author: "Scout".to_string(),
        body: "Confirmed from Trailer 2".to_string(),
        created_at: 1_752_839_500,
    };
    assert_eq!(
        serde_json::from_str::<MarkerComment>(&serde_json::to_string(&comment).unwrap()).unwrap(),
        comment
    );

    let overview = AdminDbOverview {
        landmarks: 750,
        user_markers: 12,
        comments: 34,
        challenge_scores: 18,
        news_items: 4,
        notifications: 2,
        schema_version: "v0.8".to_string(),
    };
    assert_eq!(
        serde_json::from_str::<AdminDbOverview>(&serde_json::to_string(&overview).unwrap())
            .unwrap(),
        overview
    );

    let wiki = WikiCacheEntry {
        id: "b1".to_string(),
        title: "Ritz-Carlton Bal Harbour".to_string(),
        summary: "Luxury oceanfront resort in Vice Beach.".to_string(),
        url: Some("https://gta.fandom.com/wiki/Vice_Beach".to_string()),
        image_url: None,
        updated_at: 1_752_839_600,
    };
    assert_eq!(
        serde_json::from_str::<WikiCacheEntry>(&serde_json::to_string(&wiki).unwrap()).unwrap(),
        wiki
    );
}
