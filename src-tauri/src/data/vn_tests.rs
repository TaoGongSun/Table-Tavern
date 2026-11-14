use super::*;
use serde_json::json;

fn cast() -> Vec<(String, String)> {
    vec![("c1".to_owned(), "艾琳".to_owned())]
}

fn parsed(value: serde_json::Value) -> Bible {
    normalize(parse_bible(&value).unwrap(), &cast())
}

/// 壞 id、重複 id、壞條件的那筆丟掉；holder 名字對成 id、對不到成 null；旗標型別各自修正
#[test]
fn normalize_drops_bad_items_and_fixes_shapes() {
    let bible = parsed(json!({
        "chapter_max": "40",
        "flags": [
            {"id": "met", "type": "bool", "initial": "yes"},
            {"id": "met", "type": "bool", "initial": true},
            {"id": "Bad-Id", "type": "bool"},
            {"id": "trust", "type": "counter", "initial": 50, "min": 0, "max": 10},
            {"id": "route", "type": "route", "values": ["a"], "initial": "a"},
            {"id": "path", "type": "route", "values": ["a", "b", "a"], "initial": "z"},
            {"id": "broken", "type": "nope"}
        ],
        "secrets": [
            {"id": "s1", "holder": "艾琳", "reveal": {"flag": "met"}},
            {"id": "s2", "holder": "路人", "reveal": {"flag": "ghost"}},
            {"id": "s3", "reveal": {"chapter": "x"}}
        ],
        "endings": [
            {"id": "good", "when": [{"kind": "range", "path": "flags.trust", "min": 7}]},
            {"id": "bad", "when": [{"kind": "range", "path": "hp", "min": 1}]},
            {"id": "odd", "when": [{"kind": "what"}]}
        ]
    }));
    assert_eq!(bible.chapter_max, 30);
    let flags: Vec<(&str, &str)> = bible
        .flags
        .iter()
        .map(|flag| (flag.id.as_str(), flag.initial.as_str()))
        .collect();
    assert_eq!(flags, [("met", "false"), ("trust", "10"), ("path", "a")]);
    assert_eq!(bible.flags[2].values, ["a", "b"]);
    assert_eq!(bible.secrets[0].holder.as_deref(), Some("c1"));
    assert_eq!(bible.secrets[1].holder, None);
    assert_eq!(bible.secrets[1].reveal, Reveal::default());
    assert_eq!(bible.secrets[2].reveal, Reveal::default());
    let endings: Vec<&str> = bible
        .endings
        .iter()
        .map(|ending| ending.id.as_str())
        .collect();
    assert_eq!(endings, ["good", DEFAULT_ENDING]);
}

/// 缺章數用保底、太小夾到 3；default 一定在且無條件；線性作品自動補章門檻（已有就不補）
#[test]
fn normalize_chapter_max_default_ending_and_linear_gate() {
    assert_eq!(parsed(json!({})).chapter_max, CHAPTER_FALLBACK);
    assert_eq!(parsed(json!({"chapter_max": 1})).chapter_max, 3);
    let bible = parsed(json!({
        "branching": "none",
        "chapter_max": 5,
        "flags": [{"id": "met", "type": "bool"}],
        "endings": [
            {"id": "default", "when": [{"kind": "flag", "path": "flags.met", "expect": true}]},
            {"id": "met_end", "when": [{"kind": "flag", "path": "flags.met", "expect": true}]},
            {"id": "late", "when": [{"kind": "range", "path": "chapter", "min": 4}]}
        ]
    }));
    assert!(bible.endings[0].when.is_empty());
    let gate = Condition::Range {
        path: "chapter".to_owned(),
        min: Some(5.0),
        max: None,
        min_exclusive: false,
        max_exclusive: false,
        default: None,
    };
    assert_eq!(bible.endings[1].when.last(), Some(&gate));
    assert_eq!(bible.endings[2].when.len(), 1);
    let mut moved = bible.clone();
    moved.chapter_max = 9;
    move_chapter_gate(&mut moved, 5);
    assert!(
        matches!(&moved.endings[1].when[1], Condition::Range { min: Some(min), .. } if *min == 9.0)
    );
}

/// 常駐段 12,000、單章 4,000 字超出截短
#[test]
fn normalize_caps_long_text() {
    let long = "字".repeat(13_000);
    let bible = parsed(json!({
        "logline": long,
        "style": "文風",
        "chapters": [{"title": "一", "beats": "拍".repeat(5_000), "plant": ["伏筆"]}]
    }));
    assert_eq!(bible.logline.chars().count(), RESIDENT_CAP);
    assert!(bible.style.is_empty());
    assert_eq!(bible.chapters[0].beats.chars().count(), CHAPTER_CAP);
    assert!(bible.chapters[0].plant.is_empty());
}

/// 補種只補缺的、不覆蓋現值；規則每條 inject turn、bool 與 route 帶合法值
#[test]
fn seeds_fill_only_missing_and_rules_carry_allowed() {
    let bible = parsed(json!({
        "flags": [
            {"id": "met", "type": "bool"},
            {"id": "trust", "type": "counter", "initial": 2, "min": 0, "max": 10},
            {"id": "route", "type": "route", "values": ["a", "b"]}
        ],
        "secrets": [{"id": "s1"}]
    }));
    let mut tree = BTreeMap::new();
    super::super::set_tree_value(&mut tree, &["flags".to_owned(), "trust".to_owned()], "7");
    assert!(fill_missing(&mut tree, &bible.seeds()));
    let leaf = |path: &[&str]| {
        let path: Vec<String> = path.iter().map(|part| part.to_string()).collect();
        super::super::node_at(&tree, &path).cloned()
    };
    assert_eq!(
        leaf(&["flags", "trust"]),
        Some(StateNode::Leaf("7".to_owned()))
    );
    assert_eq!(
        leaf(&["flags", "met"]),
        Some(StateNode::Leaf("false".to_owned()))
    );
    assert_eq!(
        leaf(&["revealed", "s1"]),
        Some(StateNode::Leaf("false".to_owned()))
    );
    assert_eq!(leaf(&["chapter"]), Some(StateNode::Leaf("1".to_owned())));
    assert!(!fill_missing(&mut tree, &bible.seeds()));

    let rules = bible.rules();
    assert!(rules.values().all(|rule| rule.inject == InjectLevel::Turn));
    assert_eq!(rules["flags.met"].allowed, ["true", "false"]);
    assert_eq!(rules["flags.route"].allowed, ["a", "b"]);
    assert_eq!(rules["flags.trust"].kind, FieldKind::Counter);
    assert_eq!(rules["revealed.s1"].allowed, ["true", "false"]);
    assert_eq!(rules["chapter"].update, UpdateMode::Reject);
}

/// 引擎三鍵（包 5 §2.1）：沒聖經的桌補規則與受保護根一次就好；聖經的種子與規則也帶這三鍵；
/// 結局 id 不得是 `none`（`ended` 的未完結值）
#[test]
fn engine_keys_seed_rule_and_reserve_none() {
    let mut mechanism = super::super::Mechanism::default();
    assert!(ensure_engine_rules(&mut mechanism));
    assert!(!ensure_engine_rules(&mut mechanism));
    assert_eq!(mechanism.locked_roots.len(), LOCKED_ROOTS.len());
    assert!(["chapter", "ended", "finale"]
        .iter()
        .all(|key| mechanism.rules[*key].update == UpdateMode::Reject));
    let bible = parsed(json!({"endings": [{"id": "none"}, {"id": "good"}]}));
    let ids: Vec<&str> = bible
        .endings
        .iter()
        .map(|ending| ending.id.as_str())
        .collect();
    assert_eq!(ids, ["good", DEFAULT_ENDING]);
    let mut tree = BTreeMap::new();
    fill_missing(&mut tree, &bible.seeds());
    assert_eq!(
        (chapter_in(&tree), ended_in(&tree), finale_in(&tree)),
        (1, None, false)
    );
    assert!(bible.rules().contains_key("ended"));
}

/// 偏好壓過模型：章數夾進長度區間、分支照玩家選的；沒選就照模型
#[test]
fn preferences_override_chapter_range_and_branching() {
    let mut bible = Bible {
        chapter_max: 12,
        branching: Branching::Full,
        ..Bible::default()
    };
    Preferences {
        length: Some(Length::Short),
        branching: Some(Branching::None),
        ..Preferences::default()
    }
    .apply(&mut bible);
    assert_eq!((bible.chapter_max, bible.branching), (5, Branching::None));
    Preferences::default().apply(&mut bible);
    assert_eq!((bible.chapter_max, bible.branching), (5, Branching::None));
}

/// 主角一句話只落在 preferences：往返保留；舊 vn.json 沒這欄讀回來是空字串
#[test]
fn preferences_tagline_round_trips_and_legacy_vn_json_defaults_empty() {
    let file = VnFile {
        preferences: Some(Preferences {
            tagline: "新來的燈塔看守".to_owned(),
            ..Preferences::default()
        }),
        ..VnFile::default()
    };
    let back: VnFile = serde_json::from_slice(&serde_json::to_vec(&file).unwrap()).unwrap();
    assert_eq!(back.preferences.unwrap().tagline, "新來的燈塔看守");
    let legacy: VnFile =
        serde_json::from_str(r#"{"version":1,"preferences":{"genres":["戀愛"],"name":"阿哲"}}"#)
            .unwrap();
    let preferences = legacy.preferences.unwrap();
    assert_eq!(
        (preferences.name.as_str(), preferences.tagline.as_str()),
        ("阿哲", "")
    );
}

#[test]
fn art_defaults_to_unstarted_and_only_round_trips_two_fields() {
    let mut file: VnFile = serde_json::from_str("{}").unwrap();
    assert!(!file.art.started);
    assert!(file.art.appeared.is_empty());
    file.art.started = true;
    file.art.appeared.push("actor".to_owned());
    let encoded = serde_json::to_value(&file).unwrap();
    assert_eq!(
        encoded["art"],
        serde_json::json!({"started":true,"appeared":["actor"]})
    );
    assert_eq!(serde_json::from_value::<VnFile>(encoded).unwrap(), file);
}

/// 玩家畫風存檔前清洗：控制字元換空白、去頭尾空白、截到與卡作者欄位相同的上限；空的不寫進檔
#[test]
fn player_style_is_cleaned_and_capped() {
    let mut art = ArtFlow::default();
    let saved = PlayerStyle {
        style: format!(" 水彩\n手繪{} ", "長".repeat(700)),
        avoid: "\t文字 ".to_owned(),
    }
    .apply(&mut art);
    assert!(art.style.starts_with("水彩 手繪"));
    assert_eq!(art.style.chars().count(), super::super::vn_art::STYLE_CAP);
    assert_eq!(art.avoid, "文字");
    assert_eq!(
        (saved.style, saved.avoid),
        (art.style.clone(), art.avoid.clone())
    );
    PlayerStyle::default().apply(&mut art);
    let encoded = serde_json::to_value(&art).unwrap();
    assert!(encoded.get("style").is_none() && encoded.get("avoid").is_none());
}
