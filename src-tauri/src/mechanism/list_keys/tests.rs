//! takeover-yaml-key-glyph：清單欄 YAML 鍵改回原卡寫法。
use super::scan::scan;
use super::*;
use crate::data::FieldRule;
use crate::mechanism::apply::apply_updates;
use crate::mechanism::test_support::{mechanism_with, tree_from};
use crate::mechanism::types::Outcome;

const SEED: &str = "    - 人物:\n        名字: \"👤 林远\"\n        行动: \"📝 牵马\"\n        体力: \"💪 55\"\n    - 人物:\n        名字: \"👤 贺铮\"\n        行动: \"📝 抽烟斗\"\n        体力: \"💪 70\"";

const GUIDE: &str = "每回合必報：\n- 在場人物：只列此刻在場的人物，照下面的 YAML 寫法：\n    - 人物:\n        名字: \"👤 姓名\"\n        行动: \"📝 当前在做什么\"\n        体力: \"💪 0-100\"\n        内心: \"💭 想法\"\n  每個值都用雙引號包住。\n- 行動選項：每條一行：\n      - \"1. 稳妥\"\n";

fn list_mechanism(guide: &str, seed: Option<&str>) -> Mechanism {
    let mut mechanism = mechanism_with(&[
        ("在場人物", FieldRule::for_kind(FieldKind::List)),
        ("行動選項", FieldRule::for_kind(FieldKind::List)),
        ("備註", FieldRule::for_kind(FieldKind::Text)),
    ]);
    mechanism.guide = guide.to_owned();
    if let Some(seed) = seed {
        mechanism
            .list_key_seeds
            .insert(vec!["在場人物".to_owned()], seed.to_owned());
    }
    mechanism
}

fn replace(path: &str, value: &str) -> Patch {
    Patch {
        op: PatchOp::Replace,
        path: path.split('.').map(str::to_owned).collect(),
        from: Vec::new(),
        value: Some(serde_json::Value::String(value.to_owned())),
    }
}

fn run(
    tree: &mut BTreeMap<String, StateNode>,
    mechanism: &Mechanism,
    patches: &[Patch],
) -> Outcome {
    apply_updates(tree, mechanism, patches)
}

fn leaf<'a>(tree: &'a BTreeMap<String, StateNode>, path: &str) -> &'a str {
    let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
    match data::node_at(tree, &segments) {
        Some(StateNode::Leaf(text)) => text,
        other => panic!("{path} 不是葉子：{other:?}"),
    }
}

fn ignored(outcome: &Outcome) -> Vec<&Record> {
    outcome
        .records
        .iter()
        .filter(|record| record.kind == RecordKind::Ignored)
        .collect()
}

// ---- 掃描子集 ----

#[test]
fn scan_finds_keys_with_mapping_ids_per_item() {
    let sites = scan(SEED).unwrap();
    let keys: Vec<&str> = sites
        .iter()
        .map(|site| &SEED[site.start..site.end])
        .collect();
    assert_eq!(
        keys,
        ["人物", "名字", "行动", "体力", "人物", "名字", "行动", "体力"]
    );
    // 第一位的三個欄位同一個映射，第二位另一個
    assert_eq!(sites[1].mapping, sites[3].mapping);
    assert_ne!(sites[1].mapping, sites[5].mapping);
    assert_eq!(sites[2].line, 3);
}

#[test]
fn scan_skips_block_scalar_and_quoted_content() {
    let text = "內心: |\n  行動: 等待\n  體力: 3\n名字: \"行動: 不是鍵\"\n";
    let sites = scan(text).unwrap();
    let keys: Vec<&str> = sites
        .iter()
        .map(|site| &text[site.start..site.end])
        .collect();
    assert_eq!(keys, ["內心", "名字"]);
}

#[test]
fn scan_gives_up_on_unsupported_yaml() {
    for text in [
        "名字: '林远'",
        "人物: {行动: 走}",
        "人物: [行动]",
        "名字: \"跨行\n  還沒收\"",
        "\"行动\": 走",
        "名字: &a 林远",
        "\t行动: 走",
        "名字: 林远\n  行动: 走",
        "名字: 林远\n多行純量",
        "---\n行动: 走",
    ] {
        assert!(scan(text).is_none(), "應放棄：{text:?}");
    }
    // 全形冒號不算鍵尾：序列項目的普通純量
    assert_eq!(scan("- 行動：走").unwrap(), Vec::new());
}

// ---- 套用 ----

#[test]
fn single_drifted_key_is_restored_and_values_stay_byte_identical() {
    let mechanism = list_mechanism(GUIDE, Some(SEED));
    let mut tree = tree_from(&[("在場人物", SEED)]);
    let drifted = SEED.replacen("行动: \"📝 抽烟斗\"", "行動: \"📝 抽煙斗：行動中\"", 1);
    let outcome = run(&mut tree, &mechanism, &[replace("在場人物", &drifted)]);
    let expected = SEED.replacen("行动: \"📝 抽烟斗\"", "行动: \"📝 抽煙斗：行動中\"", 1);
    assert_eq!(leaf(&tree, "在場人物"), expected);
    let records = ignored(&outcome);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].path, "在場人物");
    assert!(
        records[0].detail.contains("第 7 行 行動→行动（依初始值）"),
        "{}",
        records[0].detail
    );
    // 校正不回饋模型
    assert!(outcome.notes.is_empty());
}

#[test]
fn normalization_is_idempotent() {
    let mechanism = list_mechanism(GUIDE, Some(SEED));
    let mut tree = tree_from(&[("在場人物", SEED)]);
    let outcome = run(&mut tree, &mechanism, &[replace("在場人物", SEED)]);
    assert_eq!(leaf(&tree, "在場人物"), SEED);
    assert!(ignored(&outcome).is_empty());
}

#[test]
fn unsupported_yaml_is_written_verbatim() {
    let mechanism = list_mechanism(GUIDE, Some(SEED));
    for text in [
        "    - 人物:\n        名字: '林遠'\n        行動: 走",
        "    - 人物: {行動: 走}",
        "    - 人物:\n        內心: \"跨行\n          行動: 等待\"",
    ] {
        let mut tree = tree_from(&[("在場人物", SEED)]);
        let outcome = run(&mut tree, &mechanism, &[replace("在場人物", text)]);
        assert_eq!(leaf(&tree, "在場人物"), text);
        assert!(ignored(&outcome).is_empty(), "{text:?}");
    }
}

#[test]
fn block_scalar_lines_are_values_not_keys() {
    let mechanism = list_mechanism(GUIDE, Some(SEED));
    let mut tree = tree_from(&[("在場人物", SEED)]);
    let text = "    - 人物:\n        名字: \"x\"\n        內心: |\n          行動: 等待\n        體力: \"💪 1\"";
    run(&mut tree, &mechanism, &[replace("在場人物", text)]);
    // 內心 在 guide 有參考（内心）會改；區塊底下的「行動: 等待」是值不動；體力 改回 体力
    assert_eq!(
        leaf(&tree, "在場人物"),
        "    - 人物:\n        名字: \"x\"\n        内心: |\n          行動: 等待\n        体力: \"💪 1\""
    );
}

#[test]
fn exact_spelling_wins_and_ambiguous_class_is_left_alone() {
    // 參考層同一折疊類兩種寫法（行动／行動）：整類移出，模型寫什麼就留什麼
    let seed = "- 行动: a\n  名字: b\n- 行動: c\n  名字: d";
    let mechanism = list_mechanism("", Some(seed));
    let mut tree = tree_from(&[("在場人物", seed)]);
    let text = "- 行動: x\n  名字: y\n- 行動: z\n  名字: w";
    let outcome = run(&mut tree, &mechanism, &[replace("在場人物", text)]);
    assert_eq!(leaf(&tree, "在場人物"), text);
    assert!(ignored(&outcome).is_empty());
}

#[test]
fn rename_that_would_duplicate_a_key_in_the_same_mapping_is_skipped() {
    let mechanism = list_mechanism(GUIDE, Some(SEED));
    let mut tree = tree_from(&[("在場人物", SEED)]);
    // 第一位同時有 行动 與 行動：改了會重複；第二位只有 行動，照改
    let text = "- 人物:\n    行动: a\n    行動: b\n- 人物:\n    行動: c";
    let outcome = run(&mut tree, &mechanism, &[replace("在場人物", text)]);
    assert_eq!(
        leaf(&tree, "在場人物"),
        "- 人物:\n    行动: a\n    行動: b\n- 人物:\n    行动: c"
    );
    assert_eq!(ignored(&outcome).len(), 1);
    // 同一映射兩個不同寫法都要改成同一個（髮／發 折疊後相撞）：都不改
    let seed = "- 头发: a\n- 头发: b";
    let mechanism = list_mechanism("", Some(seed));
    let text = "- 人物:\n    頭髮: a\n    頭發: b";
    let mut tree = tree_from(&[("在場人物", seed)]);
    run(&mut tree, &mechanism, &[replace("在場人物", text)]);
    assert_eq!(leaf(&tree, "在場人物"), text);
}

#[test]
fn guide_fills_keys_the_seed_lacks_and_seed_wins_conflicts() {
    // 初始值只有一位（每個鍵一次，不當固定欄位），參考全靠 guide
    let single = "    - 人物:\n        名字: \"林远\"\n        行动: \"走\"";
    let mechanism = list_mechanism(GUIDE, Some(single));
    let mut tree = tree_from(&[("在場人物", single)]);
    let outcome = run(
        &mut tree,
        &mechanism,
        &[replace(
            "在場人物",
            "    - 人物:\n        內心: \"x\"\n        行動: \"y\"",
        )],
    );
    assert_eq!(
        leaf(&tree, "在場人物"),
        "    - 人物:\n        内心: \"x\"\n        行动: \"y\""
    );
    assert!(ignored(&outcome)[0].detail.contains("依欄位說明範本"));
    // 初始值寫 行動（繁），guide 寫 行动（簡）：以初始值為準
    let seed = SEED.replace("行动", "行動");
    let mechanism = list_mechanism(GUIDE, Some(&seed));
    let mut tree = tree_from(&[("在場人物", seed.as_str())]);
    run(&mut tree, &mechanism, &[replace("在場人物", SEED)]);
    assert_eq!(leaf(&tree, "在場人物"), seed);
}

#[test]
fn current_value_is_the_last_fallback_and_poisoned_when_already_drifted() {
    // 沒有初始值、沒有 guide：目前值重複出現的鍵當參考
    let mechanism = list_mechanism("", None);
    let mut tree = tree_from(&[("在場人物", SEED)]);
    let outcome = run(
        &mut tree,
        &mechanism,
        &[replace("在場人物", "- 人物:\n    行動: a\n    體力: b")],
    );
    assert_eq!(leaf(&tree, "在場人物"), "- 人物:\n    行动: a\n    体力: b");
    assert!(ignored(&outcome)[0].detail.contains("依目前值"));
    // 目前值本身已混了兩種寫法：那一類不猜
    let drifted = SEED.replacen("行动", "行動", 1);
    let mut tree = tree_from(&[("在場人物", drifted.as_str())]);
    run(
        &mut tree,
        &mechanism,
        &[replace("在場人物", "- 人物:\n    行動: a")],
    );
    assert_eq!(leaf(&tree, "在場人物"), "- 人物:\n    行動: a");
}

#[test]
fn already_drifted_table_heals_from_seed_or_guide() {
    let drifted = SEED.replacen("行动", "行動", 1);
    for mechanism in [list_mechanism("", Some(SEED)), list_mechanism(GUIDE, None)] {
        let mut tree = tree_from(&[("在場人物", drifted.as_str())]);
        run(&mut tree, &mechanism, &[replace("在場人物", &drifted)]);
        assert_eq!(leaf(&tree, "在場人物"), SEED);
    }
}

#[test]
fn single_occurrence_keys_are_not_treated_as_schema() {
    // 人名當鍵（只出現一次）不進參考：模型把 贺铮 寫成 賀錚 不改
    let current = "贺铮:\n  好感: 3\n林远:\n  好感: 5";
    let mechanism = list_mechanism("", None);
    let mut tree = tree_from(&[("在場人物", current)]);
    run(
        &mut tree,
        &mechanism,
        &[replace("在場人物", "賀錚:\n  好感: 4")],
    );
    assert_eq!(leaf(&tree, "在場人物"), "賀錚:\n  好感: 4");
}

#[test]
fn references_are_fixed_before_the_batch() {
    // 第一筆把目前值改成繁體鍵，第二筆仍以整批開始前的目前值為參考
    let mechanism = list_mechanism("", None);
    let mut tree = tree_from(&[("在場人物", SEED)]);
    let traditional = SEED.replace("行动", "行動").replace("体力", "體力");
    let outcome = run(
        &mut tree,
        &mechanism,
        &[
            replace("在場人物", &traditional),
            replace("在場人物", &traditional),
        ],
    );
    assert_eq!(leaf(&tree, "在場人物"), SEED);
    assert_eq!(ignored(&outcome).len(), 2);
}

#[test]
fn only_explicit_list_fields_are_touched() {
    let mechanism = list_mechanism(GUIDE, Some(SEED));
    let text = "- 人物:\n    行動: a";
    // 一般文字欄
    let mut tree = tree_from(&[("在場人物", SEED), ("備註", "x")]);
    run(&mut tree, &mechanism, &[replace("備註", text)]);
    assert_eq!(leaf(&tree, "備註"), text);
    // 沒有規則（依現值推定）的欄位
    let mut tree = tree_from(&[("在場人物", SEED), ("雜項", "x")]);
    run(&mut tree, &mechanism, &[replace("雜項", text)]);
    assert_eq!(leaf(&tree, "雜項"), text);
}

#[test]
fn wildcard_list_rule_counts_as_explicit() {
    let mut mechanism = mechanism_with(&[("隊伍.*", FieldRule::for_kind(FieldKind::List))]);
    mechanism
        .list_key_seeds
        .insert(vec!["隊伍".to_owned(), "甲".to_owned()], SEED.to_owned());
    let mut tree = tree_from(&[("隊伍.甲", SEED)]);
    run(
        &mut tree,
        &mechanism,
        &[replace("隊伍.甲", "- 人物:\n    行動: a")],
    );
    assert_eq!(leaf(&tree, "隊伍.甲"), "- 人物:\n    行动: a");
}

#[test]
fn insert_onto_existing_and_new_list_fields_is_normalized() {
    let mut mechanism = list_mechanism(GUIDE, Some(SEED));
    mechanism
        .rules
        .insert("隊伍.*".to_owned(), FieldRule::for_kind(FieldKind::List));
    let insert = |path: &str, value: &str| Patch {
        op: PatchOp::Insert,
        ..replace(path, value)
    };
    let mut tree = tree_from(&[("在場人物", SEED), ("隊伍.甲", SEED)]);
    let outcome = run(
        &mut tree,
        &mechanism,
        &[
            insert("在場人物", "- 人物:\n    行動: a"),
            insert("隊伍.乙", "- 人物:\n    體力: b\n- 人物:\n    體力: c"),
        ],
    );
    assert_eq!(leaf(&tree, "在場人物"), "- 人物:\n    行动: a");
    // 全新的欄位沒有初始值與 guide，參考是整批開始前的目前值——新欄位沒有目前值，原字保留
    assert_eq!(
        leaf(&tree, "隊伍.乙"),
        "- 人物:\n    體力: b\n- 人物:\n    體力: c"
    );
    assert_eq!(ignored(&outcome).len(), 1);
    // 新欄位對得上初始值時照改
    mechanism
        .list_key_seeds
        .insert(vec!["隊伍".to_owned(), "丙".to_owned()], SEED.to_owned());
    run(
        &mut tree,
        &mechanism,
        &[insert("隊伍.丙", "- 人物:\n    體力: b")],
    );
    assert_eq!(leaf(&tree, "隊伍.丙"), "- 人物:\n    体力: b");
}

#[test]
fn rejected_write_leaves_no_record() {
    let mut rule = FieldRule::for_kind(FieldKind::List);
    rule.update = data::UpdateMode::Reject;
    let mut mechanism = mechanism_with(&[("在場人物", rule)]);
    mechanism
        .list_key_seeds
        .insert(vec!["在場人物".to_owned()], SEED.to_owned());
    let mut tree = tree_from(&[("在場人物", SEED)]);
    let outcome = run(
        &mut tree,
        &mechanism,
        &[replace("在場人物", "- 人物:\n    行動: a")],
    );
    assert_eq!(leaf(&tree, "在場人物"), SEED);
    assert!(ignored(&outcome).is_empty());
}

fn keys_only(keys: Option<Vec<(Vec<String>, String)>>) -> Option<Vec<String>> {
    keys.map(|keys| keys.into_iter().map(|(_, key)| key).collect())
}

fn field(path: &str) -> Vec<String> {
    path.split('.').map(str::to_owned).collect()
}

#[test]
fn guide_that_cannot_be_bound_or_conflicts_is_not_used() {
    let fields = [field("在場人物")];
    let other_field = "- 道具：\n    - 名稱: x\n      行动: y\n";
    assert_eq!(guide_keys(other_field, &fields[0], &fields), None);
    let twice = format!("{GUIDE}- 在場人物：另一份\n    - 人物:\n        行動: x\n");
    assert_eq!(guide_keys(&twice, &fields[0], &fields), None);
    assert_eq!(
        keys_only(guide_keys(GUIDE, &fields[0], &fields)),
        Some(
            ["人物", "名字", "行动", "体力", "内心"]
                .map(str::to_owned)
                .to_vec()
        )
    );
}

#[test]
fn guide_last_segment_binds_only_when_unique_and_full_path_wins() {
    let guide = "- 人物：\n    - 名字: a\n      行动: b\n";
    // 甲.人物 與 乙.人物 末段同名：不採用
    let fields = [field("甲.人物"), field("乙.人物")];
    assert_eq!(guide_keys(guide, &fields[0], &fields), None);
    // 只有一個欄位叫 人物：末段可以
    let fields = [field("甲.人物")];
    assert_eq!(
        keys_only(guide_keys(guide, &fields[0], &fields)),
        Some(["名字", "行动"].map(str::to_owned).to_vec())
    );
    // 完整路徑的條目優先於末段同名的條目（即使末段不唯一）
    let guide = format!("{guide}- 甲.人物：\n    - 名字: a\n      體力: c\n");
    let fields = [field("甲.人物"), field("乙.人物")];
    assert_eq!(
        keys_only(guide_keys(&guide, &fields[0], &fields)),
        Some(["名字", "體力"].map(str::to_owned).to_vec())
    );
    // 套用層：乙.人物 不吃 `- 人物：` 的範本，繁體鍵照原字
    let mut mechanism = mechanism_with(&[
        ("甲.人物", FieldRule::for_kind(FieldKind::List)),
        ("乙.人物", FieldRule::for_kind(FieldKind::List)),
    ]);
    mechanism.guide = "- 人物：\n    - 名字: a\n      行动: b\n".to_owned();
    let mut tree = tree_from(&[("甲.人物", "- 名字: x"), ("乙.人物", "- 名字: y")]);
    run(
        &mut tree,
        &mechanism,
        &[replace("乙.人物", "- 名字: z\n  行動: w")],
    );
    assert_eq!(leaf(&tree, "乙.人物"), "- 名字: z\n  行動: w");
}

#[test]
fn same_folded_key_in_another_branch_is_not_touched() {
    let seed = "人物列表:\n  - 人物:\n      行动: a\n  - 人物:\n      行动: b\n備註:\n  行動: 原本就是繁體";
    let mechanism = list_mechanism("", Some(seed));
    let mut tree = tree_from(&[("在場人物", seed)]);
    let text = "人物列表:\n  - 人物:\n      行動: x\n  - 人物:\n      行動: y\n備註:\n  行動: 照舊";
    let outcome = run(&mut tree, &mechanism, &[replace("在場人物", text)]);
    assert_eq!(
        leaf(&tree, "在場人物"),
        "人物列表:\n  - 人物:\n      行动: x\n  - 人物:\n      行动: y\n備註:\n  行動: 照舊"
    );
    assert_eq!(ignored(&outcome).len(), 1);
    // 參考在另一個分支（備註 底下）也不會跨過來：人物欄位的位置沒有參考就不改
    let seed = "備註:\n  行动: a\n其他:\n  行动: b";
    let mechanism = list_mechanism("", Some(seed));
    let mut tree = tree_from(&[("在場人物", seed)]);
    let text = "- 人物:\n    行動: x\n- 人物:\n    行動: y";
    run(&mut tree, &mechanism, &[replace("在場人物", text)]);
    assert_eq!(leaf(&tree, "在場人物"), text);
}

#[test]
fn scan_records_structural_positions() {
    let text = "人物列表:\n  - 人物:\n      行动: a\n備註:\n  行動: b";
    let sites = scan(text).unwrap();
    let located: Vec<(Vec<&str>, &str)> = sites
        .iter()
        .map(|site| {
            (
                site.parent.iter().map(String::as_str).collect(),
                &text[site.start..site.end],
            )
        })
        .collect();
    assert_eq!(
        located,
        [
            (vec![], "人物列表"),
            (vec!["人物列表", "[]"], "人物"),
            (vec!["人物列表", "[]", "人物"], "行动"),
            (vec![], "備註"),
            (vec!["備註"], "行動"),
        ]
    );
}

#[test]
fn seeds_reload_from_saved_refactor_outcome() {
    let root = std::env::temp_dir().join(format!("tt-list-keys-{}", ulid::Ulid::generate()));
    std::fs::create_dir_all(&root).unwrap();
    let world_id = data::create_world(&root, "驛站").unwrap();
    let card = serde_json::json!({
        "format": "table-tavern-refactor-card",
        "version": 1,
        "outcome": { "interface": { "state_fields": { "在場人物": SEED, "馬匹": 40 } } }
    });
    data::write_refactor_outcome(&root, &world_id, &card.to_string()).unwrap();
    let seeds = resolve_list_key_seeds(&root, &world_id);
    assert_eq!(seeds.len(), 1);
    assert_eq!(seeds[&vec!["在場人物".to_owned()]], SEED);
    // 不落檔：存回 state.json 再讀不帶這份，要靠載入層重算
    let mut mechanism = list_mechanism("", None);
    mechanism.list_key_seeds = seeds;
    let reloaded: Mechanism =
        serde_json::from_str(&serde_json::to_string(&mechanism).unwrap()).unwrap();
    assert!(reloaded.list_key_seeds.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn drifted_top_level_path_still_gets_nested_keys_fixed() {
    // 樹與規則是簡體「在场人物」，模型連路徑都寫成繁體：路徑先對回，值裡的鍵照樣改
    let mut mechanism = mechanism_with(&[("在场人物", FieldRule::for_kind(FieldKind::List))]);
    mechanism
        .list_key_seeds
        .insert(vec!["在场人物".to_owned()], SEED.to_owned());
    let mut tree = tree_from(&[("在场人物", SEED)]);
    let outcome = run(
        &mut tree,
        &mechanism,
        &[replace("在場人物", "- 人物:\n    行動: a")],
    );
    assert_eq!(leaf(&tree, "在场人物"), "- 人物:\n    行动: a");
    let records = ignored(&outcome);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].path, "在场人物");
}
