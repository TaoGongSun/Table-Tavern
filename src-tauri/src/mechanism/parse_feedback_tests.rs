//! mechanism-error-feedback：解析層認不出的更新不再靜默丟掉，照原位置記拒收回饋 GM。
use super::parse::parse_block;
use super::types::{Parsed, PatchOp};

fn details(items: &[Parsed]) -> Vec<String> {
    items
        .iter()
        .map(|item| match item {
            Parsed::Patch(patch) => format!("PATCH {}", patch.path.join(".")),
            Parsed::Note(record) => record.detail.clone(),
        })
        .collect()
}

fn single_patch(block: &str) -> super::types::Patch {
    match parse_block(block).as_slice() {
        [Parsed::Patch(patch)] => patch.clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn add_is_insert_like_upstream() {
    let patch = single_patch(r#"[{"op":"add","path":"/物品/-","value":"劍"}]"#);
    assert_eq!(patch.op, PatchOp::Insert);
    assert_eq!(patch.path, vec!["物品", "-"]);
}

#[test]
fn move_target_is_path_then_to() {
    let only_to = single_patch(r#"[{"op":"move","from":"/a","to":"/b"}]"#);
    assert_eq!(only_to.path, vec!["b"]);
    let both = single_patch(r#"[{"op":"move","from":"/a","path":"/p","to":"/t"}]"#);
    assert_eq!(both.path, vec!["p"]);
    let null_path = single_patch(r#"[{"op":"move","from":"/a","path":null,"to":"/t"}]"#);
    assert_eq!(null_path.path, vec!["t"]);
    let items = parse_block(r#"[{"op":"move","from":"/a","path":5,"to":"/t"}]"#);
    assert_eq!(
        details(&items),
        vec!["move 的 path 不是字串（to /t），未套用。"]
    );
}

#[test]
fn every_unreadable_object_is_rejected_in_place() {
    let block = r#"[
        {"op":"replace","path":"/a","value":1},
        "字串",
        {"path":"/b","value":1},
        {"op":5,"path":"/c"},
        {"op":"set","path":"/d","value":1},
        {"op":"replace","value":1},
        {"op":"delta","path":7,"value":1},
        {"op":"move","to":"/e"},
        {"op":"move","from":"/f"},
        {"op":"replace","path":"/g","value":2}
    ]"#;
    let items = parse_block(block);
    let ops = "op 只能是 replace、delta、insert、remove、move";
    assert_eq!(
        details(&items),
        vec![
            "PATCH a".to_owned(),
            "更新區塊裡有一筆不是 JSON 物件（\"字串\"），未套用。".to_owned(),
            format!("有一筆更新缺 op 或 op 不是字串（path /b），未套用；{ops}。"),
            format!("有一筆更新缺 op 或 op 不是字串（path /c），未套用；{ops}。"),
            format!("op「set」無法辨識（path /d），未套用；{ops}。"),
            "replace 缺 path 或 path 不是字串，未套用。".to_owned(),
            "delta 缺 path 或 path 不是字串，未套用。".to_owned(),
            "move 缺來源路徑 from（to /e），未套用。".to_owned(),
            "move 缺目的地 path（from /f），未套用。".to_owned(),
            "PATCH g".to_owned(),
        ]
    );
    let Parsed::Note(record) = &items[2] else {
        panic!()
    };
    assert_eq!(record.path, "/b", "抓得到 path 就記在那條路徑");
}

#[test]
fn broken_object_keeps_the_good_ones_and_says_not_to_resend() {
    let block = r#"[{"op":"replace","path":"/a","value":1} {"op":"replace","path":"/b","value":}, {"op":"replace","path":"/c","value":3}]"#;
    assert_eq!(
        details(&parse_block(block)),
        vec![
            "PATCH a",
            "本輪更新區塊有 1 筆格式壞掉未套用（JSON 語法錯誤）：path /b；其餘已處理（被擋下的另列回饋），已套用的不要重送。",
            "PATCH c",
        ]
    );
}

#[test]
fn unclosed_tail_is_reported_at_the_end_of_the_block() {
    let block =
        r#"[{"op":"replace","path":"/a","value":1}, {"op":"replace","path":"/b","value":"未收"#;
    assert_eq!(
        details(&parse_block(block)),
        vec![
            "PATCH a",
            "本輪更新區塊從 path /b 起格式壞掉（JSON 語法錯誤，多半是字串少了收尾引號），前面的已處理（被擋下的另列回饋）；這筆起到結尾的更新都未套用，請重送。",
        ]
    );
}

#[test]
fn broken_object_without_a_path_quotes_its_opening() {
    let block = r#"{"op":"replace" "value":1}"#;
    assert_eq!(
        details(&parse_block(block)),
        vec!["本輪更新區塊有 1 筆格式壞掉未套用（JSON 語法錯誤）：「{\"op\":\"replace\" \"value\":1}」。"]
    );
}

#[test]
fn content_without_any_json_object_gets_the_no_jsonpatch_note() {
    let note = "更新區塊裡沒有可解析的 JSONPatch 陣列，整段未套用。";
    // `_.set` 指令式寫法（不含大括號）與 YAML 條列
    for block in [
        "_.set('HP', 50, 45);//受傷",
        "- op: replace\n  path: /a\n  value: 1",
    ] {
        assert_eq!(details(&parse_block(block)), vec![note], "{block}");
    }
}

#[test]
fn empty_array_and_analysis_only_record_nothing() {
    assert!(parse_block("[]").is_empty());
    assert!(parse_block("<JSONPatch>[]</JSONPatch>").is_empty());
    assert!(parse_block("<Analysis>這回合沒什麼變化</Analysis>").is_empty());
    assert!(parse_block("   ").is_empty());
}

/// 實測（Haiku，2026-10-12）：清單欄字串尾端寫成 `\"}` 少了收尾引號，之後六筆全被吞掉——不能說「其餘已處理」。
#[test]
fn unclosed_string_swallowing_later_objects_asks_for_a_resend() {
    let block = r#"[
  {"op":"replace","path":"/a","value":"1"},
  {"op":"replace","path":"/list","value":"- x: \"y\"},
  {"op":"replace","path":"/b","value":"2"},
  {"op":"replace","path":"/c","value":"3"}
]"#;
    let items = parse_block(block);
    let texts = details(&items);
    assert_eq!(texts[0], "PATCH a");
    assert_eq!(texts.len(), 2, "{texts:?}");
    assert!(
        texts[1].starts_with("本輪更新區塊從 path /list 起格式壞掉"),
        "{}",
        texts[1]
    );
    assert!(texts[1].contains("請重送") && !texts[1].contains("其餘已處理"));
}

#[test]
fn balanced_broken_object_plus_unclosed_tail_gives_two_notes_without_false_reassurance() {
    let block = r#"[{"op":"replace","path":"/a","value":1} {"op":"replace","path":"/b","value":}, {"op":"replace","path":"/c","value":"未收"#;
    let texts = details(&parse_block(block));
    assert_eq!(texts.len(), 3, "{texts:?}");
    assert_eq!(texts[0], "PATCH a");
    assert_eq!(
        texts[1],
        "本輪更新區塊有 1 筆格式壞掉未套用（JSON 語法錯誤）：path /b。"
    );
    assert!(texts[2].starts_with("本輪更新區塊從 path /c 起格式壞掉"));
}

/// 壞 JSON 與套用時被擋（路徑不存在）同一區塊：兩條回饋不互相矛盾。
#[test]
fn broken_json_note_does_not_claim_blocked_updates_were_applied() {
    use super::apply_block;
    use super::test_support::{mechanism_with, world_with};
    let mut world = world_with(&[("A.x", "1")], mechanism_with(&[]));
    let block = crate::transport::StateBlock {
        fields: Vec::new(),
        updates: vec![r#"<JSONPatch>[{"op":"delta","path":"/A/沒有","value":1} {"op":"replace","path":"/A/x","value":}]</JSONPatch>"#.to_owned()],
        display: String::new(),
        ..Default::default()
    };
    apply_block(&mut world, &block, "阿濤");
    assert_eq!(
        world.state.notes,
        vec![
            "路徑不存在：/A/沒有。上一層現有：x。若要新增請用 insert。".to_owned(),
            "本輪更新區塊有 1 筆格式壞掉未套用（JSON 語法錯誤）：path /A/x；其餘已處理（被擋下的另列回饋），已套用的不要重送。".to_owned(),
        ]
    );
    assert!(world
        .state
        .notes
        .iter()
        .all(|note| !note.contains("其餘已套用")));
}
