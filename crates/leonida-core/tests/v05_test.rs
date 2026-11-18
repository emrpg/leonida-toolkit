use leonida_core::{
    companion_stats, compute_ng_key_index, compute_segment_size, export_handling_meta,
    filter_collectibles, joaat, parse_handling_meta, parse_rage_header, parse_rpf_header,
    parse_rsc7_header, CollectibleItem, CoreError, HandlingItem, RageHeader, RpfEncryption,
    RpfVersion, RPF7_MAGIC, RPF8_MAGIC, RSC7_MAGIC,
};

#[test]
fn v05_rage_rpf7_rpf8_header_parsing() {
    let mut rpf7 = Vec::new();
    rpf7.extend_from_slice(&RPF7_MAGIC.to_le_bytes());
    rpf7.extend_from_slice(&42u32.to_le_bytes());
    rpf7.extend_from_slice(&512u32.to_le_bytes());
    rpf7.extend_from_slice(&0x0FFF_FFFFu32.to_le_bytes());

    let parsed = parse_rpf_header(&rpf7).expect("valid RPF7 header");
    assert_eq!(parsed.version, RpfVersion::Rpf7);
    assert_eq!(parsed.entry_count, 42);
    assert_eq!(parsed.names_length, 512);
    assert_eq!(parsed.encryption, RpfEncryption::Ng);

    let mut rpf8 = Vec::new();
    rpf8.extend_from_slice(&RPF8_MAGIC.to_le_bytes());
    rpf8.extend_from_slice(&100u32.to_le_bytes());
    rpf8.extend_from_slice(&1024u32.to_le_bytes());
    rpf8.extend_from_slice(&0x4E45_504Fu32.to_le_bytes());

    let parsed8 = parse_rpf_header(&rpf8).expect("valid RPF8 header");
    assert_eq!(parsed8.version, RpfVersion::Rpf8);
    assert_eq!(parsed8.encryption, RpfEncryption::Open);

    assert!(matches!(
        parse_rpf_header(&[0x52, 0x50, 0x46]),
        Err(CoreError::InvalidHeader(_))
    ));
    assert!(matches!(
        parse_rpf_header(&[0u8; 16]),
        Err(CoreError::InvalidHeader(_))
    ));
}

#[test]
fn v05_rage_rsc7_header_and_segment_size() {
    assert_eq!(compute_segment_size(0), 0);
    // 1 page of base (0x2000 = 8192) at bit 17, shift = 0
    assert_eq!(compute_segment_size(1 << 17), 8192);
    // Shift = 1 (base = 16384), 2 pages at bit 17
    assert_eq!(compute_segment_size((2 << 17) | 1), 32768);
    // Max u32 never panics
    let _ = compute_segment_size(u32::MAX);

    let mut rsc7 = Vec::new();
    rsc7.extend_from_slice(&RSC7_MAGIC.to_le_bytes());
    rsc7.extend_from_slice(&13u32.to_le_bytes());
    rsc7.extend_from_slice(&(1u32 << 17).to_le_bytes());
    rsc7.extend_from_slice(&(2u32 << 17).to_le_bytes());

    let hdr = parse_rsc7_header(&rsc7).expect("valid RSC7 header");
    assert_eq!(hdr.version, 13);
    assert_eq!(hdr.system_size, 8192);
    assert_eq!(hdr.graphics_size, 16384);

    assert!(matches!(
        parse_rage_header(&rsc7),
        Ok(RageHeader::Rsc7(h)) if h.version == 13
    ));
}

#[test]
fn v05_rage_joaat_and_ng_key_index() {
    assert_eq!(joaat(""), 0);
    assert_eq!(joaat("adder"), 0xB779_A091);
    assert_eq!(joaat("ADDER"), 0xB779_A091);
    assert_eq!(joaat("zentorno"), 0xAC5D_F515);

    let idx = compute_ng_key_index("common.rpf", 1024);
    assert!(idx < 101);
    let idx_max = compute_ng_key_index("x64a.rpf", u64::MAX);
    assert!(idx_max < 101);
}

#[test]
fn v05_rage_handling_meta_round_trip() {
    let items = vec![HandlingItem {
        handling_name: "CHEETAH".to_string(),
        f_mass: 1450.0,
        f_initial_drag_coeff: 7.5,
        f_drive_bias_front: 0.0,
        n_initial_drive_gears: 6,
        f_initial_drive_force: 0.36,
        f_drive_max_flat_vel: 165.0,
        f_brake_force: 1.15,
        f_traction_curve_max: 2.45,
        f_suspension_force: 2.1,
    }];

    let xml = export_handling_meta(&items);
    let parsed = parse_handling_meta(&xml).expect("valid handling.meta XML");
    assert_eq!(parsed, items);

    assert!(matches!(
        parse_handling_meta("<Item><handlingName>BAD</handlingName><fMass value=\"abc\" /></Item>"),
        Err(CoreError::InvalidHandlingMeta(_))
    ));
}

#[test]
fn v05_companion_stats_and_filter() {
    let items = vec![
        CollectibleItem {
            id: "c1".to_string(),
            title: "Vice Beach Stunt Jump".to_string(),
            category: "stunt".to_string(),
            district: "Vice Beach".to_string(),
            x: 1340.0,
            y: 2830.0,
            completed: true,
        },
        CollectibleItem {
            id: "c2".to_string(),
            title: "Port Vice Package".to_string(),
            category: "package".to_string(),
            district: "Port Vice".to_string(),
            x: -500.0,
            y: -900.0,
            completed: false,
        },
    ];

    let stats = companion_stats(&items);
    assert_eq!(stats.total, 2);
    assert_eq!(stats.completed, 1);
    assert!((stats.percent - 50.0).abs() < 1e-9);

    let empty_stats = companion_stats(&[]);
    assert_eq!(empty_stats.percent, 0.0);

    let done = filter_collectibles(&items, None, Some(true));
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].id, "c1");

    let packages = filter_collectibles(&items, Some("package"), None);
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].id, "c2");
}
