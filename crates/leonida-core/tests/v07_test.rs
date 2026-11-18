use leonida_core::{filter_natives, generate_shv_stub, native_stats, NativeEntry, NativeParam};

fn sample_entries() -> Vec<NativeEntry> {
    vec![
        NativeEntry {
            namespace: "PLAYER".to_string(),
            name: "PLAYER_PED_ID".to_string(),
            hash: "0xD80958FC74E988A6".to_string(),
            jhash: Some("0xFA92E226".to_string()),
            params: vec![],
            return_type: "Ped".to_string(),
            description: "Returns current player ped handle.".to_string(),
        },
        NativeEntry {
            namespace: "ENTITY".to_string(),
            name: "SET_ENTITY_COORDS".to_string(),
            hash: "0x06843DA7060A026B".to_string(),
            jhash: Some("0xDF70B41B".to_string()),
            params: vec![
                NativeParam {
                    name: "entity".to_string(),
                    param_type: "Entity".to_string(),
                },
                NativeParam {
                    name: "xPos".to_string(),
                    param_type: "float".to_string(),
                },
                NativeParam {
                    name: "yPos".to_string(),
                    param_type: "float".to_string(),
                },
                NativeParam {
                    name: "zPos".to_string(),
                    param_type: "float".to_string(),
                },
            ],
            return_type: "void".to_string(),
            description: "Teleports an entity to world coordinates.".to_string(),
        },
        NativeEntry {
            namespace: "ENTITY".to_string(),
            name: "GET_ENTITY_COORDS".to_string(),
            hash: "0x3FEF770D40960D5A".to_string(),
            jhash: None,
            params: vec![
                NativeParam {
                    name: "entity".to_string(),
                    param_type: "Entity".to_string(),
                },
                NativeParam {
                    name: "alive".to_string(),
                    param_type: "BOOL".to_string(),
                },
            ],
            return_type: "Vector3".to_string(),
            description: "Gets the current world position of an entity.".to_string(),
        },
    ]
}

#[test]
fn v07_natives_filter_and_stats() {
    let entries = sample_entries();

    let stats = native_stats(&entries);
    assert_eq!(stats.len(), 2);
    assert_eq!(stats[0].namespace, "ENTITY");
    assert_eq!(stats[0].count, 2);
    assert_eq!(stats[1].namespace, "PLAYER");
    assert_eq!(stats[1].count, 1);

    let by_hash = filter_natives(&entries, Some("0x3FEF770D40960D5A"), None, 10);
    assert_eq!(by_hash.len(), 1);
    assert_eq!(by_hash[0].name, "GET_ENTITY_COORDS");

    let by_ns = filter_natives(&entries, Some("coords"), Some("entity"), 1);
    assert_eq!(by_ns.len(), 1);

    assert!(filter_natives(&entries, None, None, 0).is_empty());
}

#[test]
fn v07_natives_generate_shv_stubs() {
    let entries = sample_entries();
    let ped_id = &entries[0];
    let set_coords = &entries[1];

    let cpp_ret = generate_shv_stub(ped_id, "cpp");
    assert!(cpp_ret.contains("return invoke<Ped>(0xD80958FC74E988A6);"));

    let cpp_void = generate_shv_stub(set_coords, "c++");
    assert!(cpp_void.contains("invoke<Void>(0x06843DA7060A026B, entity, xPos, yPos, zPos);"));

    let csharp = generate_shv_stub(set_coords, "csharp");
    assert!(csharp.contains("Function.Call((Hash)0x06843DA7060A026B, entity, xPos, yPos, zPos);"));

    let rust = generate_shv_stub(ped_id, "rust");
    assert!(rust.contains("pub fn player_ped_id() -> i32"));
    assert!(rust.contains("native_invoke(0xD80958FC74E988A6, &[])"));

    let unknown = generate_shv_stub(ped_id, "python");
    assert!(unknown.contains("Unsupported target language"));
}

#[test]
fn v07_natives_serde_round_trip() {
    let entries = sample_entries();
    let json = serde_json::to_string(&entries).unwrap();
    let decoded: Vec<NativeEntry> = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, entries);
}
