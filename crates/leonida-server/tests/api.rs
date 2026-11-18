use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use leonida_server::core_adapter::CoreEngine;
use leonida_server::db::{MarkerStore, MARKERS_SCHEMA};
use leonida_server::natives::{NativeEntry, NativeParam};
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

fn sample_natives() -> Vec<NativeEntry> {
    vec![
        NativeEntry {
            namespace: "PLAYER".into(),
            name: "GET_PLAYER_PED".into(),
            hash: "0x43A66C31C68491C0".into(),
            jhash: Some("0x6E31E993".into()),
            params: vec![NativeParam {
                name: "player".into(),
                param_type: "Player".into(),
            }],
            return_type: "Ped".into(),
            description: "Gets the ped for a specified player index.".into(),
        },
        NativeEntry {
            namespace: "VEHICLE".into(),
            name: "CREATE_VEHICLE".into(),
            hash: "0xAF35D0D2583051B0".into(),
            jhash: None,
            params: vec![],
            return_type: "Vehicle".into(),
            description: "Creates a vehicle at the given coordinates.".into(),
        },
        NativeEntry {
            namespace: "PLAYER".into(),
            name: "SET_PLAYER_INVINCIBLE".into(),
            hash: "0x239528EACDC3E7DE".into(),
            jhash: None,
            params: vec![],
            return_type: "void".into(),
            description: "Sets whether a player is invincible.".into(),
        },
    ]
}

fn test_app_with_dirs(
    dist_dir: PathBuf,
    tiles_dir: PathBuf,
    data_dir: PathBuf,
    phone_dist_dir: PathBuf,
) -> axum::Router {
    test_app_with_all_dirs(
        dist_dir,
        tiles_dir,
        data_dir,
        phone_dist_dir,
        std::env::temp_dir(),
    )
}

fn test_app_with_all_dirs(
    dist_dir: PathBuf,
    tiles_dir: PathBuf,
    data_dir: PathBuf,
    phone_dist_dir: PathBuf,
    natives_dist_dir: PathBuf,
) -> axum::Router {
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
        natives: Arc::new(sample_natives()),
        dist_dir,
        tiles_dir,
        data_dir,
        phone_dist_dir,
        natives_dist_dir,
    })
}

fn test_app() -> axum::Router {
    let tmp = std::env::temp_dir();
    test_app_with_dirs(tmp.clone(), tmp.clone(), tmp.clone(), tmp)
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

    let app = test_app_with_dirs(dist_dir, tiles_dir, base.clone(), base.clone());

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

#[tokio::test]
async fn photogrammetry_solve_and_projects_crud() {
    let app = test_app();

    // Known camera at (0, 0, 100), yaw=0 (north +y), pitch=0, fov=60 deg, 1280x720
    // fl = 640 / tan(30 deg) = 1108.5125168440814
    let fl = 640.0_f64 / 30.0_f64.to_radians().tan();
    let pts = [
        (-200.0, 1000.0, 150.0),
        (250.0, 1200.0, 80.0),
        (0.0, 800.0, 200.0),
        (150.0, 900.0, 40.0),
    ];
    let obs: Vec<Value> = pts
        .iter()
        .map(|&(wx, wy, wz)| {
            let u = 640.0 + fl * (wx / wy);
            let v = 360.0 - fl * ((wz - 100.0) / wy);
            json!({ "worldX": wx, "worldY": wy, "worldZ": wz, "px": u, "py": v })
        })
        .collect();

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/photogrammetry/solve")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "observations": obs,
                        "width": 1280.0,
                        "height": 720.0,
                        "fovDeg": 60.0,
                        "rangeM": 1200.0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let solved = body_json(res).await;
    assert!(solved["pose"]["rmsePx"].as_f64().unwrap() < 2.0);
    assert!(solved["frustum"].as_array().unwrap().len() >= 18);

    // Fewer than 3 observations -> 400
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/photogrammetry/solve")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "observations": [obs[0], obs[1]],
                        "width": 1280.0,
                        "height": 720.0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // Save project to SQLite
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/photogrammetry/projects")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "name": "Vice Beach Calibration",
                        "imageName": "trailer1_frame42.png",
                        "width": 1280.0,
                        "height": 720.0,
                        "observations": obs,
                        "pose": solved["pose"],
                        "frustum": solved["frustum"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let saved = body_json(res).await;
    let proj_id = saved["project"]["id"].as_str().unwrap().to_string();
    assert_eq!(saved["project"]["name"], "Vice Beach Calibration");

    // List projects
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/photogrammetry/projects")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let list = body_json(res).await;
    assert_eq!(list["projects"].as_array().unwrap().len(), 1);

    // Delete by query param
    let res = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/photogrammetry/projects?id={proj_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body_json(res).await["deleted"], true);

    // Deleting again by path -> 404
    let res = app
        .oneshot(
            Request::delete(format!("/api/photogrammetry/projects/{proj_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn route_planning_endpoint() {
    let app = test_app();

    let res = app
        .oneshot(
            Request::get("/api/route?x1=1000&y1=2000&x2=1460&y2=2655")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let route = body_json(res).await;
    let wps = route["waypoints"].as_array().unwrap();
    assert!(wps.len() >= 2);
    assert_eq!(wps[0], json!([1000.0, 2000.0]));
    assert_eq!(wps[wps.len() - 1], json!([1460.0, 2655.0]));
    assert!(route["distanceM"].as_f64().unwrap() > 0.0);
    assert!(route["driveMin"].as_f64().unwrap() > 0.0);
    assert!(route["flyMin"].as_f64().unwrap() > 0.0);
}

#[tokio::test]
async fn companion_checklist_toggle_and_export_import() {
    let app = test_app();

    // Initial checklist
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/companion/checklist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let cl = body_json(res).await;
    let total = cl["stats"]["total"].as_u64().unwrap();
    assert!(total >= 5);
    assert_eq!(cl["stats"]["completed"], 0);

    let first_id = cl["items"][0]["id"].as_str().unwrap().to_string();

    // Toggle first item to completed=true
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/companion/toggle")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "id": first_id, "completed": true }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let toggled = body_json(res).await;
    assert_eq!(toggled["completed"], true);
    assert_eq!(toggled["stats"]["completed"], 1);

    // Export progress
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/companion/export")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(res.headers().contains_key(header::CONTENT_DISPOSITION));
    let exported = body_json(res).await;
    assert_eq!(exported["schema"], "leonida-companion-v1");
    assert_eq!(exported["completedIds"][0], first_id);

    // Import additional completed item
    let second_id = cl["items"][1]["id"].as_str().unwrap().to_string();
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/companion/import")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "schema": "leonida-companion-v1",
                        "completedIds": [second_id]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let imp = body_json(res).await;
    assert_eq!(imp["stats"]["completed"], 2);
}

#[tokio::test]
async fn rage_inspect_header_and_handling_roundtrip() {
    let app = test_app();

    // RPF7 header: magic 52 50 46 37, entry_count=42, names_length=512, encryption=OPEN (4E 45 50 4F)
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/rage/inspect-header")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "hex": "52 50 46 37 00 00 00 2A 00 00 02 00 4E 45 50 4F" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let rpf = body_json(res).await;
    assert_eq!(rpf["format"], "RPF7");
    assert_eq!(rpf["entryCount"], 42);
    assert_eq!(rpf["namesLength"], 512);
    assert_eq!(rpf["encryption"], "Open");

    // RSC7 header: magic 52 53 43 37 ("RSC7" LE: 0x37435352), version=13, sys_flags=0x00020000, gfx_flags=0x00040000
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/rage/inspect-header")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "hex": "52534337 0D000000 00000200 00000400" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let rsc = body_json(res).await;
    assert_eq!(rsc["format"], "RSC7");
    assert_eq!(rsc["version"], 13);
    assert!(rsc["systemSize"].as_u64().unwrap() > 0);

    // handling.meta parse -> export roundtrip
    let sample_xml = r#"<CHandlingDataMgr><HandlingData>
        <Item type="CHandlingData">
            <handlingName>CHEETAH</handlingName>
            <fMass value="1300.0" />
            <fInitialDragCoeff value="8.5" />
            <fDriveBiasFront value="0.2" />
            <nInitialDriveGears value="6" />
            <fInitialDriveForce value="0.38" />
            <fDriveMaxFlatVel value="175.0" />
            <fBrakeForce value="1.1" />
            <fTractionCurveMax value="2.6" />
            <fSuspensionForce value="2.4" />
        </Item>
    </HandlingData></CHandlingDataMgr>"#;

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/rage/handling/parse")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "xml": sample_xml }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let parsed = body_json(res).await;
    assert_eq!(parsed["items"][0]["handlingName"], "CHEETAH");
    assert_eq!(parsed["items"][0]["nInitialDriveGears"], 6);

    let res = app
        .oneshot(
            Request::post("/api/rage/handling/export")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "items": parsed["items"] }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let exported = body_json(res).await;
    let xml_out = exported["xml"].as_str().unwrap();
    assert!(xml_out.contains("<handlingName>CHEETAH</handlingName>"));
    assert!(xml_out.contains("<fMass value=\"1300.000000\" />"));
}

#[tokio::test]
async fn phone_contacts_messages_notes_photos_crud() {
    let app = test_app();

    // Contacts
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/phone/contacts")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let contacts = body_json(res).await;
    assert_eq!(contacts["contacts"].as_array().unwrap().len(), 5);

    // Messages
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/phone/messages")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "contactId": "lucia", "text": "Heading to Vice Beach" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/phone/messages?contact_id=lucia")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let msgs = body_json(res).await;
    assert_eq!(msgs["messages"].as_array().unwrap().len(), 2);

    // Notes CRUD
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/phone/notes")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "title": "Boat Dock",
                        "body": "Check at midnight",
                        "x": 1200.0,
                        "y": 2400.0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let note_id = body_json(res).await["note"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/phone/notes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body_json(res).await["notes"].as_array().unwrap().len(), 1);

    let res = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/phone/notes?id={note_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/phone/notes/{note_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // Photos CRUD
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/phone/photos")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "title": "Ocean Drive Sunset",
                        "district": "Vice Beach",
                        "x": 1400.0,
                        "y": 2700.0,
                        "filter": "warm",
                        "caption": "Palm trees",
                        "imageDataUrl": "data:image/png;base64,iVBORw0KGgo="
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let photo_id = body_json(res).await["photo"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/phone/photos")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body_json(res).await["photos"].as_array().unwrap().len(), 1);

    let res = app
        .clone()
        .oneshot(
            Request::delete(format!("/api/phone/photos/{photo_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .oneshot(
            Request::delete(format!("/api/phone/photos/{photo_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn phone_static_hosting() {
    let base = std::env::temp_dir().join(format!("leonida-phone-test-{}", uuid::Uuid::new_v4()));
    let phone_dist = base.join("phone_dist");
    std::fs::create_dir_all(&phone_dist).unwrap();
    std::fs::write(
        phone_dist.join("index.html"),
        "<html>LeonidaOS Phone</html>",
    )
    .unwrap();

    let app = test_app_with_dirs(base.clone(), base.clone(), base.clone(), phone_dist);

    let res = app
        .oneshot(Request::get("/phone/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"<html>LeonidaOS Phone</html>");

    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test]
async fn sol_auth_api_public_and_markers() {
    let app = test_app();

    for action in [
        "access_config",
        "gtadb_source_public",
        "site_settings_public",
        "ranks",
    ] {
        let res = app
            .clone()
            .oneshot(
                Request::get(format!("/auth/api.php?action={action}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "action {action} failed");
        let json = body_json(res).await;
        assert_eq!(json["ok"], true);
    }

    // access_verify
    let res = app
        .clone()
        .oneshot(
            Request::post("/auth/api.php?action=access_verify")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body_json(res).await["verified"], true);

    // notifications & notifications_read
    let res = app
        .clone()
        .oneshot(
            Request::get("/auth/api.php?action=notifications")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let notif = body_json(res).await;
    assert_eq!(notif["unreadCount"], 1);

    let res = app
        .clone()
        .oneshot(
            Request::post("/auth/api.php?action=notifications_read")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body_json(res).await["updated"], 1);

    // marker_save -> marker_list -> marker_update -> marker_delete
    let res = app
        .clone()
        .oneshot(
            Request::post("/auth/api.php?action=marker_save")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "name": "SOL Pin", "x": 100.0, "y": -200.0 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let saved = body_json(res).await;
    let marker_id = saved["marker"]["id"].as_str().unwrap().to_string();

    let res = app
        .clone()
        .oneshot(
            Request::get("/auth/api.php?action=marker_list")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body_json(res).await["markers"].as_array().unwrap().len(), 1);

    let res = app
        .clone()
        .oneshot(
            Request::post("/auth/api.php?action=marker_update")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "id": marker_id, "name": "SOL Pin Updated" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body_json(res).await["marker"]["name"], "SOL Pin Updated");

    let res = app
        .clone()
        .oneshot(
            Request::post("/auth/api.php?action=marker_delete")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "id": marker_id }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body_json(res).await["deleted"], true);

    // Unknown action -> 400
    let res = app
        .oneshot(
            Request::get("/auth/api.php?action=does_not_exist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn sol_challenge_submit_and_leaderboard() {
    let app = test_app();

    // Exact hit -> 5000, grade S
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/sol/challenge/submit")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "player": "Lucia",
                        "entryId": "t1-01",
                        "targetX": 1000.0,
                        "targetY": 2000.0,
                        "guessX": 1000.0,
                        "guessY": 2000.0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let sub1 = body_json(res).await;
    assert_eq!(sub1["result"]["score"], 5000);
    assert_eq!(sub1["result"]["grade"], "S");

    // Farther guess -> lower score
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/sol/challenge/submit")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "player": "Jason",
                        "entryId": "t1-01",
                        "targetX": 1000.0,
                        "targetY": 2000.0,
                        "guessX": 4000.0,
                        "guessY": 2000.0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let res = app
        .clone()
        .oneshot(
            Request::get("/auth/api.php?action=leaderboard&game=screenshot_challenge&limit=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let lb = body_json(res).await;
    let entries = lb["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["player"], "Lucia");
    assert_eq!(entries[0]["score"], 5000);

    let res = app
        .oneshot(
            Request::get("/auth/api.php?action=game_stats&game=screenshot_challenge")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let gs = body_json(res).await;
    assert_eq!(gs["totalPlays"], 2);
    assert_eq!(gs["bestScore"], 5000);
}

#[tokio::test]
async fn sol_media_files_and_static_data() {
    let base = std::env::temp_dir().join(format!("leonida-sol-test-{}", uuid::Uuid::new_v4()));
    let dist_dir = base.join("dist");
    let tiles_dir = base.join("tiles");
    let data_dir = base.join("data");
    std::fs::create_dir_all(&dist_dir).unwrap();
    std::fs::create_dir_all(&tiles_dir).unwrap();
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::write(
        data_dir.join("trailer1.json"),
        r#"[{"id":"t1","title":"Vice Beach Shot"}]"#,
    )
    .unwrap();

    let app = test_app_with_dirs(dist_dir, tiles_dir, data_dir, base.clone());

    let res = app
        .clone()
        .oneshot(
            Request::get("/get-media-files.php?action=tabs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let tabs = body_json(res).await;
    assert!(!tabs["tabs"].as_array().unwrap().is_empty());

    let res = app
        .clone()
        .oneshot(
            Request::get("/get-media-files.php?type=trailer1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let files = body_json(res).await;
    assert_eq!(files["files"][0]["id"], "t1");

    let res = app
        .clone()
        .oneshot(
            Request::get("/get-media-files.php?type=badtype")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let res = app
        .oneshot(
            Request::get("/data/trailer1.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let raw = body_json(res).await;
    assert_eq!(raw[0]["title"], "Vice Beach Shot");

    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test]
async fn natives_search_stats_bookmarks_and_static_hosting() {
    let base = std::env::temp_dir().join(format!("leonida-nat-test-{}", uuid::Uuid::new_v4()));
    let natives_dist = base.join("natives_dist");
    std::fs::create_dir_all(&natives_dist).unwrap();
    std::fs::write(natives_dist.join("index.html"), "<html>NativeDB</html>").unwrap();

    let app = test_app_with_all_dirs(
        base.clone(),
        base.clone(),
        base.clone(),
        base.clone(),
        natives_dist,
    );

    // Search by query & namespace
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/natives?q=ped&ns=PLAYER&limit=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let list = body_json(res).await;
    assert_eq!(list["total"], 1);
    assert_eq!(list["items"][0]["name"], "GET_PLAYER_PED");

    // Stats
    let res = app
        .clone()
        .oneshot(
            Request::get("/api/natives/stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let stats = body_json(res).await;
    assert_eq!(stats["total"], 3);
    assert_eq!(stats["namespaces"][0]["namespace"], "PLAYER");
    assert_eq!(stats["namespaces"][0]["count"], 2);

    // Bookmarks CRUD
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/natives/bookmarks")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "hash": "0x43A66C31C68491C0",
                        "namespace": "PLAYER",
                        "name": "GET_PLAYER_PED",
                        "note": "Primary player ped lookup"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let res = app
        .clone()
        .oneshot(
            Request::get("/api/natives/bookmarks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bms = body_json(res).await;
    assert_eq!(bms["bookmarks"].as_array().unwrap().len(), 1);
    assert_eq!(bms["bookmarks"][0]["note"], "Primary player ped lookup");

    let res = app
        .clone()
        .oneshot(
            Request::delete("/api/natives/bookmarks?hash=0x43A66C31C68491C0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .clone()
        .oneshot(
            Request::delete("/api/natives/bookmarks/0x43A66C31C68491C0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // Static /natives/
    let res = app
        .oneshot(Request::get("/natives/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"<html>NativeDB</html>");

    let _ = std::fs::remove_dir_all(&base);
}

#[tokio::test]
async fn sol_marker_discussion_and_save_changelog() {
    let app = test_app();

    let res = app
        .clone()
        .oneshot(
            Request::post("/auth/api.php?action=marker_discussion_post")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "markerId": "b1",
                        "author": "Lucia",
                        "body": "Confirmed oceanfront hotel facade."
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let posted = body_json(res).await;
    assert_eq!(posted["comment"]["markerId"], "b1");
    assert_eq!(posted["comment"]["author"], "Lucia");

    let res = app
        .clone()
        .oneshot(
            Request::get("/auth/api.php?action=marker_discussion_list&marker_id=b1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let list = body_json(res).await;
    assert_eq!(list["comments"].as_array().unwrap().len(), 1);

    let res = app
        .clone()
        .oneshot(
            Request::get("/auth/api.php?action=marker_discussion_stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let stats = body_json(res).await;
    assert_eq!(stats["totalComments"], 1);
    assert_eq!(stats["discussedMarkers"], 1);

    let res = app
        .oneshot(
            Request::post("/save-changelog.php")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!([{ "version": "0.8.0", "note": "Added discussion & admin studio" }])
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let cl = body_json(res).await;
    assert_eq!(cl["saved"], true);
}

#[tokio::test]
async fn sol_admin_studio_endpoints() {
    let app = test_app();

    // Seed one challenge score so admin_users has an entry
    let _ = app
        .clone()
        .oneshot(
            Request::post("/api/sol/challenge/submit")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "player": "Jason",
                        "targetX": 100.0,
                        "targetY": 100.0,
                        "guessX": 100.0,
                        "guessY": 100.0
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    for action in [
        "admin_stats",
        "admin_db_overview",
        "admin_db_integrity",
        "admin_db_export",
        "admin_news",
        "admin_site_settings",
        "admin_credits",
        "admin_ranks",
        "admin_users",
    ] {
        let res = app
            .clone()
            .oneshot(
                Request::get(format!("/auth/api.php?action={action}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "admin action {action} failed");
        assert_eq!(body_json(res).await["ok"], true);
    }

    // Save news
    let res = app
        .clone()
        .oneshot(
            Request::post("/auth/api.php?action=admin_news_save")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "title": "Patch 0.8", "body": "All systems operational." }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body_json(res).await["item"]["title"], "Patch 0.8");

    // Save site settings and read back
    let res = app
        .clone()
        .oneshot(
            Request::post("/auth/api.php?action=admin_site_settings_save")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "accentColor": "#00f5d4", "panelStyle": "solid" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let res = app
        .oneshot(
            Request::get("/auth/api.php?action=admin_site_settings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body_json(res).await["settings"]["accentColor"], "#00f5d4");
}
