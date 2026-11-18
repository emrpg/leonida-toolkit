use leonida_core::{
    filter_sol_entries, group_sol_entries_by_dataset, parse_sol_entries,
    score_screenshot_challenge, sol_dataset_stats, ChallengeGrade, ChallengeScoreResult,
    ChangelogEntry, CoreError, NewsItem, NotificationItem, RankInfo, SiteSettings, SolEntry,
    SolEntryFilter,
};

#[test]
fn sol_score_screenshot_challenge_grades_and_bounds() {
    let exact = score_screenshot_challenge(100.0, 200.0, 100.0, 200.0);
    assert_eq!(exact.distance_m, 0.0);
    assert_eq!(exact.score, 5000);
    assert_eq!(exact.grade, ChallengeGrade::S);

    let grade_a = score_screenshot_challenge(0.0, 0.0, 300.0, 0.0);
    assert_eq!(grade_a.score, 3894);
    assert_eq!(grade_a.grade, ChallengeGrade::A);

    let grade_b = score_screenshot_challenge(0.0, 0.0, 800.0, 0.0);
    assert_eq!(grade_b.score, 2567);
    assert_eq!(grade_b.grade, ChallengeGrade::B);

    let grade_c = score_screenshot_challenge(0.0, 0.0, 1200.0, 0.0);
    assert_eq!(grade_c.score, 1839);
    assert_eq!(grade_c.grade, ChallengeGrade::C);

    let grade_d = score_screenshot_challenge(0.0, 0.0, 3000.0, 4000.0);
    assert_eq!(grade_d.distance_m, 5000.0);
    assert_eq!(grade_d.score, 78);
    assert_eq!(grade_d.grade, ChallengeGrade::D);

    let non_finite = score_screenshot_challenge(0.0, 0.0, f64::NAN, 0.0);
    assert_eq!(non_finite.score, 0);
    assert_eq!(non_finite.grade, ChallengeGrade::D);
}

#[test]
fn sol_parse_stats_group_and_filter_entries() {
    let json = r#"[
        {
            "id": "t1-01",
            "title": "Ocean View Hotel",
            "category": "landmark",
            "dataset": "trailer1",
            "x": 1420.5,
            "y": 2810.0,
            "district": "Vice Beach",
            "status": "confirmed",
            "timestampSec": 14.5,
            "mediaUrl": "/media/t1_01.jpg",
            "realWorld": "Ocean Drive, Miami Beach"
        },
        {
            "id": "sc-02",
            "title": "Port Vice Crane",
            "category": "infrastructure",
            "dataset": "screenshots",
            "x": -900.0,
            "y": -1200.0,
            "z": 45.0,
            "district": "Port Vice",
            "status": "unconfirmed",
            "description": "Gantry cranes at sunset"
        },
        {
            "id": "lk-03",
            "title": "Metro Station",
            "category": "landmark",
            "dataset": "leaks",
            "x": 100.0,
            "y": -200.0
        }
    ]"#;

    let entries: Vec<SolEntry> = parse_sol_entries(json).expect("valid sol JSON");
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].timestamp_sec, Some(14.5));
    assert_eq!(entries[2].district, None);

    assert!(matches!(
        parse_sol_entries("{not valid json"),
        Err(CoreError::Json(_))
    ));

    let stats = sol_dataset_stats(&entries);
    assert_eq!(stats.total, 3);
    assert_eq!(stats.by_dataset.get("trailer1"), Some(&1));
    assert_eq!(stats.by_dataset.get("screenshots"), Some(&1));
    assert_eq!(stats.by_dataset.get("leaks"), Some(&1));
    assert_eq!(stats.by_category.get("landmark"), Some(&2));
    assert_eq!(stats.by_district.get("(none)"), Some(&1));
    assert_eq!(stats.by_status.get("confirmed"), Some(&1));

    let grouped = group_sol_entries_by_dataset(&entries);
    assert_eq!(grouped.len(), 3);
    assert_eq!(grouped["trailer1"][0].id, "t1-01");

    let filtered = filter_sol_entries(
        &entries,
        &SolEntryFilter {
            text: Some("miami hotel".to_string()),
            dataset: Some("trailer1".to_string()),
            ..Default::default()
        },
    );
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, "t1-01");

    let by_cat = filter_sol_entries(
        &entries,
        &SolEntryFilter {
            category: Some("landmark".to_string()),
            ..Default::default()
        },
    );
    assert_eq!(by_cat.len(), 2);
}

#[test]
fn sol_models_serde_round_trip() {
    let settings = SiteSettings {
        site_title: "State of Leonida".to_string(),
        maintenance_mode: false,
        community_access_enabled: true,
        default_language: "en".to_string(),
        default_tileset: "dupzor,51".to_string(),
        motd: Some("Welcome to Leonida".to_string()),
    };
    let s_json = serde_json::to_string(&settings).unwrap();
    assert_eq!(
        serde_json::from_str::<SiteSettings>(&s_json).unwrap(),
        settings
    );

    let rank = RankInfo {
        id: "scout".to_string(),
        name: "Map Scout".to_string(),
        level: 2,
        badge_color: Some("#38bdf8".to_string()),
        description: None,
    };
    assert_eq!(
        serde_json::from_str::<RankInfo>(&serde_json::to_string(&rank).unwrap()).unwrap(),
        rank
    );

    let notif = NotificationItem {
        id: "n1".to_string(),
        title: "New Marker".to_string(),
        message: "Added Vice Beach hotel".to_string(),
        created_at: 1_752_839_001,
        read: false,
        kind: Some("info".to_string()),
    };
    assert_eq!(
        serde_json::from_str::<NotificationItem>(&serde_json::to_string(&notif).unwrap()).unwrap(),
        notif
    );

    let news = NewsItem {
        id: "news-1".to_string(),
        title: "Trailer 2 Breakdown".to_string(),
        summary: "42 new locations mapped".to_string(),
        published_at: "2025-05-10".to_string(),
        author: Some("leonida-team".to_string()),
        tags: vec!["trailer2".to_string()],
    };
    assert_eq!(
        serde_json::from_str::<NewsItem>(&serde_json::to_string(&news).unwrap()).unwrap(),
        news
    );

    let changelog = ChangelogEntry {
        version: "0.3.0".to_string(),
        date: "2026-11-18".to_string(),
        summary: "Added SOL datasets".to_string(),
        changes: vec!["Added trailer1..3".to_string()],
    };
    assert_eq!(
        serde_json::from_str::<ChangelogEntry>(&serde_json::to_string(&changelog).unwrap())
            .unwrap(),
        changelog
    );

    let score = ChallengeScoreResult {
        distance_m: 120.0,
        score: 4524,
        grade: ChallengeGrade::S,
    };
    assert_eq!(
        serde_json::from_str::<ChallengeScoreResult>(&serde_json::to_string(&score).unwrap())
            .unwrap(),
        score
    );
}
