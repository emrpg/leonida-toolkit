use leonida_core::{
    default_phone_contacts, filter_photos_by_district, messages_for_contact, phone_stats,
    unread_count, PhoneContact, PhoneMessage, PhoneNote, SnapmaticPhoto,
};

#[test]
fn v06_phone_models_serde_round_trip() {
    let contact = PhoneContact {
        id: "c1".to_string(),
        name: "Lucia".to_string(),
        role: "Partner".to_string(),
        district: "Vice Beach".to_string(),
        avatar_color: "#f43f5e".to_string(),
        bio: "Meet at Ocean View.".to_string(),
    };
    let c_json = serde_json::to_string(&contact).unwrap();
    assert_eq!(
        serde_json::from_str::<PhoneContact>(&c_json).unwrap(),
        contact
    );

    let msg = PhoneMessage {
        id: "m1".to_string(),
        contact_id: "c1".to_string(),
        sender: "Lucia".to_string(),
        text: "On my way".to_string(),
        timestamp: 1_752_839_001,
        unread: true,
    };
    let m_json = serde_json::to_string(&msg).unwrap();
    assert_eq!(serde_json::from_str::<PhoneMessage>(&m_json).unwrap(), msg);

    let photo = SnapmaticPhoto {
        id: "p1".to_string(),
        title: "Sunset Pier".to_string(),
        district: "Vice Beach".to_string(),
        x: 1340.0,
        y: 2830.0,
        filter: "warm".to_string(),
        caption: "Neon reflections".to_string(),
        image_data_url: "data:image/png;base64,AAAA".to_string(),
        created_at: 1_752_839_100,
    };
    let p_json = serde_json::to_string(&photo).unwrap();
    assert_eq!(
        serde_json::from_str::<SnapmaticPhoto>(&p_json).unwrap(),
        photo
    );

    let note = PhoneNote {
        id: "n1".to_string(),
        title: "Stunt Jump".to_string(),
        body: "Ramp behind hotel".to_string(),
        x: Some(1340.0),
        y: Some(2830.0),
        updated_at: 1_752_839_200,
    };
    let n_json = serde_json::to_string(&note).unwrap();
    assert_eq!(serde_json::from_str::<PhoneNote>(&n_json).unwrap(), note);
}

#[test]
fn v06_phone_stats_threads_and_filters() {
    let contacts = default_phone_contacts();
    assert_eq!(contacts.len(), 3);

    let messages = vec![
        PhoneMessage {
            id: "m2".to_string(),
            contact_id: "lucia".to_string(),
            sender: "you".to_string(),
            text: "Copy that".to_string(),
            timestamp: 200,
            unread: false,
        },
        PhoneMessage {
            id: "m1".to_string(),
            contact_id: "lucia".to_string(),
            sender: "Lucia".to_string(),
            text: "Check the pier".to_string(),
            timestamp: 100,
            unread: true,
        },
        PhoneMessage {
            id: "m3".to_string(),
            contact_id: "jason".to_string(),
            sender: "Jason".to_string(),
            text: "Ready at Port Vice".to_string(),
            timestamp: 150,
            unread: true,
        },
    ];

    let lucia_thread = messages_for_contact(&messages, "lucia");
    assert_eq!(lucia_thread.len(), 2);
    assert_eq!(lucia_thread[0].id, "m1");
    assert_eq!(lucia_thread[1].id, "m2");
    assert_eq!(unread_count(&messages), 2);

    let photos = vec![
        SnapmaticPhoto {
            id: "p1".to_string(),
            title: "Pier".to_string(),
            district: "Vice Beach".to_string(),
            x: 1340.0,
            y: 2830.0,
            filter: "none".to_string(),
            caption: "".to_string(),
            image_data_url: "".to_string(),
            created_at: 10,
        },
        SnapmaticPhoto {
            id: "p2".to_string(),
            title: "Cranes".to_string(),
            district: "Port Vice".to_string(),
            x: -500.0,
            y: -900.0,
            filter: "noir".to_string(),
            caption: "".to_string(),
            image_data_url: "".to_string(),
            created_at: 20,
        },
    ];

    assert_eq!(
        filter_photos_by_district(&photos, Some("vice beach")).len(),
        1
    );
    assert_eq!(filter_photos_by_district(&photos, None).len(), 2);

    let notes = vec![PhoneNote {
        id: "n1".to_string(),
        title: "Safehouse".to_string(),
        body: "Back alley".to_string(),
        x: None,
        y: None,
        updated_at: 50,
    }];

    let stats = phone_stats(&contacts, &messages, &photos, &notes);
    assert_eq!(stats.contacts, 3);
    assert_eq!(stats.messages, 3);
    assert_eq!(stats.unread_messages, 2);
    assert_eq!(stats.photos, 2);
    assert_eq!(stats.notes, 1);
}
