//! 單人接力：入口對模式、停止與換桌訊號、點名分類、上限解析。驅動本身由 relay_tests 覆蓋。
use super::*;

fn key(relay: &str) -> Key {
    ("w-solo".to_owned(), relay.to_owned())
}

#[test]
fn entries_map_to_modes_and_call_lists() {
    let (mode, step) = relay_mode("request", Some("fox".to_owned()), None).unwrap();
    assert_eq!(
        mode,
        RelayMode::Call {
            card_id: "fox".to_owned(),
            list: CallList::Omit
        }
    );
    assert_eq!(step, RelayStep::Call("fox".to_owned()));
    let (mode, _) = relay_mode("nominated", Some("fox".to_owned()), Some("n1".to_owned())).unwrap();
    assert_eq!(
        mode,
        RelayMode::Call {
            card_id: "fox".to_owned(),
            list: CallList::UnlessSince {
                narration_id: "n1".to_owned()
            }
        }
    );
    assert!(relay_mode("nominated", Some("fox".to_owned()), None).is_err());
    assert!(relay_mode("request", None, None).is_err());
    assert!(relay_mode("bogus", None, None).is_err());
    for (name, expected) in [
        ("narrate", RelayMode::Narrate),
        ("narrateThenOne", RelayMode::NarrateThenOne),
        ("advance", RelayMode::Advance),
    ] {
        let (mode, step) = relay_mode(name, None, None).unwrap();
        assert_eq!(mode, expected);
        assert_eq!(step, RelayStep::Narrate);
    }
}

#[test]
fn stop_wins_over_release_and_signals_end_with_the_relay() {
    let relay = key("r1");
    {
        signals().insert(relay.clone(), Signal::default());
        let _registration = Registration(relay.clone());
        assert_eq!(halt_of(&relay), Halt::None);
        release("w-solo", "r1");
        assert_eq!(halt_of(&relay), Halt::StopRelay);
        interrupt("w-solo", "r1");
        assert_eq!(halt_of(&relay), Halt::Interrupt);
        // 別條接力不受影響
        assert_eq!(halt_of(&key("r2")), Halt::None);
    }
    assert_eq!(halt_of(&relay), Halt::None);
    // 已結束或不存在的 id：什麼都不做
    interrupt("w-solo", "r1");
    release("w-solo", "r1");
    assert!(signals().get(&relay).is_none());
}

#[test]
fn classify_uses_player_name_and_stops_on_stage_or_missing_cards() {
    let root = crate::smart_free::test_root("solo-relay-classify");
    let world = data::create_world(&root, "酒館").unwrap();
    let card = |name: &str| data::CharacterCard {
        id: data::new_id(),
        name: name.to_owned(),
        color: "#336699".to_owned(),
        avatar: "🐺".to_owned(),
        tier: data::Tier::Balanced,
        show_image: false,
        archived: false,
        gen_prompt: String::new(),
        public_md: String::new(),
        private_md: String::new(),
        stage: false,
    };
    let wolf = card("灰狼");
    let gate = card("城門");
    data::write_character(&root, &world, &wolf).unwrap();
    data::write_character(&root, &world, &gate).unwrap();
    data::set_character_stage(&root, &world, &gate.id, true).unwrap();
    assert_eq!(
        classify(&root, &world, "", transport::PLAYER_SENTINEL),
        Nominee::Player {
            name: String::new()
        }
    );
    assert_eq!(
        classify(&root, &world, "雷克", transport::PLAYER_SENTINEL),
        Nominee::Player {
            name: "雷克".to_owned()
        }
    );
    assert_eq!(
        classify(&root, &world, "", &wolf.id),
        Nominee::Npc {
            id: wolf.id.clone(),
            name: "灰狼".to_owned()
        }
    );
    assert_eq!(classify(&root, &world, "", &gate.id), Nominee::Stop);
    assert_eq!(classify(&root, &world, "", "missing"), Nominee::Stop);
}

#[test]
fn round_cap_defaults_to_three() {
    let mut config = data::AppConfig::default();
    assert_eq!(max_speakers(&config), 3);
    config
        .preferences
        .insert("max_round_speakers".to_owned(), json!("2"));
    assert_eq!(max_speakers(&config), 2);
    config
        .preferences
        .insert("max_round_speakers".to_owned(), json!(0));
    assert_eq!(max_speakers(&config), 3);
}

#[test]
fn dice_refuses_the_gap_between_solo_relay_parts() {
    let root = crate::smart_free::test_root("dice-relay-gap");
    let world = data::create_world(&root, "雷克的骰桌").unwrap();
    crate::dice::set_enabled(&root, &world, true, "en").unwrap();
    let key = (world.clone(), "dice-relay".to_owned());
    signals().insert(key.clone(), Signal::default());
    let registration = Registration(key);
    assert_eq!(
        crate::dice::roll(&root, &world, 0, "1d20").unwrap_err(),
        "dice-relay-active"
    );
    assert_eq!(
        crate::dice::roll(&root, &world, 1, "1d20").unwrap_err(),
        "dice-scene-changed"
    );
    drop(registration);
    assert!(crate::dice::roll(&root, &world, 0, "1d20").is_ok());
    std::fs::remove_dir_all(root).unwrap();
}
