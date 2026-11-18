use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use leonida_server::core_adapter::CoreEngine;
use leonida_server::db::{MarkerStore, MARKERS_SCHEMA};
use leonida_server::{build_router, AppState};

fn sample_landmarks_json() -> &'static str {
    r#"[
        ["b1", "?, Vice Beach", [1340, 2830], 0, "The Ritz-Carlton Bal Harbour, FL", [25.898996, -80.1232991], 0, ["hotel"], "166624", 1752839001],
        ["b2", "?, Vice Beach", [1460, 2655], 0, "Akoya Condominium, FL?", [25.8460686, -80.1196967], 0, ["residential"], "b54738", 1752839001],
        ["b3", "Ocean Tower, Downtown", [1000, 2000], 1, "Biscayne Tower, Miami", [25.78, -80.19], 1, ["tower", "hotel"], "6eb6ae", 1748079313],
        ["b4", "Bayfront Mall, Downtown", [1200, 2200], 0, "Bayside Marketplace, Miami", [25.778, -80.186], 0, ["mall"], "ff00aa", 1748079314],
        ["b5", "Unknown Spot, Keys", [], 0, "?", [], 0, ["mystery"], "888888", 1748079315]
    ]"#
}

fn test_app_with_dirs(dist_dir: PathBuf, tiles_dir: PathBuf) -> axum::Router {
    let engine = Arc::new(
        CoreEngine::from_gta6map_json(sample_landmarks_json()).expect("parse sample json"),
    );
    let markers = MarkerStore::open_in_memory().expect("open in-memory sqlite");
    let tile_manifest = Arc::new(json!({
        "dupzor,51": { "0": ["0,0"] }
    }));

    build_router(AppState {
        engine,
        markers,
        tile_manifest,
        dist_dir,
        tiles_dir,
    })
}

fn test_app() -> axum::Router {
    let tmp = std::env::temp_dir();
    test_app_with_dirs(tmp.clone(), tmp)
}

async fn body_json(res: axum::response::Response) -> Value {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn health_check_works() {
    let app = test_app();
    let res = app
        .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json = body_json(res).await;
    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn landmarks_filter_and_get_by_id() {
    let app = test_app();

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/landmarks?q=Vice&tags=hotel")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let items = body_json(res).await;
    let arr = items.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], "b1");

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/landmarks?status=unconfirmed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let arr = body_json(res).await;
    assert_eq!(arr.as_array().unwrap().len(), 1);
    assert_eq!(arr[0]["id"], "b2");

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/landmarks/b3")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let item = body_json(res).await;
    assert_eq!(item["name"], "Ocean Tower");

    let res = app
        .oneshot(
            Request::get("/api/landmarks/missing")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn nearest_stats_manifest_and_geojson() {
    let app = test_app();

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/nearest?x=1340&y=2830&n=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let near = body_json(res).await;
    let arr = near.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["landmark"]["id"], "b1");
    assert_eq!(arr[0]["distance"], 0.0);

    let res = app
        .clone()
        .oneshot(Request::get("/api/stats").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let stats = body_json(res).await;
    assert_eq!(stats["total"], 5);
    assert_eq!(stats["positioned"], 4);
    assert_eq!(stats["byStatus"]["confirmed"], 3);
    assert_eq!(stats["byStatus"]["unconfirmed"], 1);
    assert_eq!(stats["byStatus"]["unknown"], 1);

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/tile-manifest")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let manifest = body_json(res).await;
    assert_eq!(manifest["dupzor,51"]["0"][0], "0,0");

    let res = app
        .oneshot(
            Request::get("/api/geojson?tags=mall")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let gj = body_json(res).await;
    assert_eq!(gj["type"], "FeatureCollection");
    assert_eq!(gj["features"].as_array().unwrap().len(), 1);
    assert_eq!(gj["features"][0]["id"], "b4");
}

#[tokio::test]
async fn predict_and_triangulate() {
    let app = test_app();

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/predict?lat=25.80&lon=-80.17")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let pred = body_json(res).await;
    assert!(pred["point"]["x"].as_f64().unwrap().is_finite());
    assert!(pred["point"]["y"].as_f64().unwrap().is_finite());
    assert_eq!(pred["neighbours"], 3);

    // Two orthogonal rays intersecting at (500, 500)
    let payload = json!({
        "rays": [
            { "origin": { "x": 0.0, "y": 500.0 }, "bearing": 90.0 },
            { "origin": { "x": 500.0, "y": 0.0 }, "bearing": 0.0 }
        ]
    });
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/triangulate")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let tri = body_json(res).await;
    assert!((tri["point"]["x"].as_f64().unwrap() - 500.0).abs() < 1e-6);
    assert!((tri["point"]["y"].as_f64().unwrap() - 500.0).abs() < 1e-6);
    assert!(tri["rms"].as_f64().unwrap() < 1e-6);

    // Parallel rays -> 422
    let parallel = json!({
        "rays": [
            { "origin": { "x": 0.0, "y": 0.0 }, "bearing": 0.0 },
            { "origin": { "x": 100.0, "y": 0.0 }, "bearing": 0.0 }
        ]
    });
    let res = app
        .oneshot(
            Request::post("/api/triangulate")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(parallel.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn user_markers_crud_and_import_export() {
    let app = test_app();

    // Initially empty
    let res = app
        .clone()
        .oneshot(Request::get("/api/markers").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let list = body_json(res).await;
    assert_eq!(list["schema"], MARKERS_SCHEMA);
    assert_eq!(list["markers"].as_array().unwrap().len(), 0);

    // Create
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/markers")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "name": "Beach Spot", "x": 123.0, "y": 456.0, "note": "sunset" })
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let created = body_json(res).await;
    let id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["name"], "Beach Spot");

    // Update
    let res = app
        .clone()
        .oneshot(
            Request::put(format!("/api/markers/{id}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "name": "Beach Spot v2", "x": 200.0 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let updated = body_json(res).await;
    assert_eq!(updated["name"], "Beach Spot v2");
    assert_eq!(updated["x"], 200.0);
    assert_eq!(updated["y"], 456.0);

    // Export
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/markers/export")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(res.headers().contains_key(header::CONTENT_DISPOSITION));
    let exported = body_json(res).await;
    assert_eq!(exported["schema"], MARKERS_SCHEMA);
    assert_eq!(exported["markers"].as_array().unwrap().len(), 1);

    // Delete single
    let res = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/markers/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Delete again -> 404
    let res = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/markers/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // Import valid bundle
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/markers/import")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "schema": MARKERS_SCHEMA,
                        "markers": [
                            { "name": "Imported A", "x": 10.0, "y": 20.0 },
                            { "name": "Imported B", "x": 30.0, "y": 40.0 }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let imp = body_json(res).await;
    assert_eq!(imp["imported"], 2);

    // Reject bad schema
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/markers/import")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "schema": "wrong-schema", "markers": [] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Clear all
    let res = app
        .oneshot(Request::delete("/api/markers").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let cleared = body_json(res).await;
    assert_eq!(cleared["deleted"], 2);
}

#[tokio::test]
async fn static_dist_and_tiles_with_cache_headers() {
    let base = std::env::temp_dir().join(format!("leonida-srv-test-{}", uuid::Uuid::new_v4()));
    let dist_dir = base.join("dist");
    let tiles_dir = base.join("tiles");
    std::fs::create_dir_all(dist_dir.clone()).unwrap();
    std::fs::create_dir_all(tiles_dir.join("dupzor,51/0")).unwrap();
    std::fs::write(dist_dir.join("index.html"), "<html>Leonida Map</html>").unwrap();
    std::fs::write(tiles_dir.join("dupzor,51/0/0,0,0.jpg"), b"fakejpg").unwrap();

    let app = test_app_with_dirs(dist_dir, tiles_dir);

    let res = app
        .clone()
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"<html>Leonida Map</html>");

    let res = app
        .oneshot(
            Request::get("/tiles/dupzor,51/0/0,0,0.jpg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers().get(header::CACHE_CONTROL).unwrap(),
        "public, max-age=86400"
    );

    let _ = std::fs::remove_dir_all(&base);
}
