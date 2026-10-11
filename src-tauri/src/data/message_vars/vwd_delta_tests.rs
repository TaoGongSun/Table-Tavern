//! mechanism-error-feedback：沒重構 MVU 卡的 `[值, 說明]` 收 delta（照上游 add 加在第 0 項），以及照索引路徑
//! 提示改寫後的持久化結果。走 GM 回合提交的路：stat_data 投影成樹 → apply_block_typed → 照帶型別合回 stat_data。
use super::convert::{merge_tree_change_typed, stat_to_tree, NewValues};
use super::json::parse;
use crate::data::{FieldKind, FieldRule, NumericUpdate};
use crate::mechanism::{Record, RecordKind};
use std::collections::BTreeMap;

fn run(
    stat: &str,
    patch: &str,
    rules: &[(&str, FieldRule)],
) -> (String, Vec<Record>, BTreeMap<String, String>) {
    let stat = parse(stat).unwrap();
    let mut world: crate::data::WorldState = serde_json::from_value(serde_json::json!({
        "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "name": "VWD 桌",
        "current_scene": 0
    }))
    .unwrap();
    world.state.tree = stat_to_tree(Some(&stat));
    world.mechanism.numeric_update = NumericUpdate::Upstream;
    for (path, rule) in rules {
        world
            .mechanism
            .rules
            .insert((*path).to_owned(), rule.clone());
    }
    let before = world.state.tree.clone();
    let reply = format!("旁白<UpdateVariable><JSONPatch>{patch}</JSONPatch></UpdateVariable>");
    let outcome = crate::mechanism::apply_block_typed(
        &mut world,
        &crate::transport::extract_state_block(&reply),
        "阿濤",
        Some(&stat),
        crate::mechanism::BlockScope::Table,
    );
    let types = BTreeMap::new();
    let rules = NewValues {
        types: &types,
        keep_strings: false,
        clean: None,
    };
    let merged = merge_tree_change_typed(
        &stat,
        &before,
        &world.state.tree,
        &rules,
        outcome.typed.as_ref(),
    );
    (merged.to_text(), outcome.records, outcome.changes)
}

fn ranged(min: f64, max: f64) -> FieldRule {
    let mut rule = FieldRule::for_kind(FieldKind::Number);
    rule.min = Some(min);
    rule.max = Some(max);
    rule
}

#[test]
fn delta_adds_to_the_first_item_and_keeps_the_description() {
    let (merged, records, changes) = run(
        r#"{"HP":[50,"生命"]}"#,
        r#"[{"op":"delta","path":"/HP","value":-10}]"#,
        &[],
    );
    assert_eq!(merged, r#"{"HP":[40,"生命"]}"#);
    assert!(records.is_empty(), "{records:?}");
    assert_eq!(changes.get("HP").map(String::as_str), Some("-10"));
    // 數字字串照收（比上游寬鬆）
    let (merged, _, _) = run(
        r#"{"HP":[50,"生命"]}"#,
        r#"[{"op":"delta","path":"/HP","value":"5"}]"#,
        &[],
    );
    assert_eq!(merged, r#"{"HP":[55,"生命"]}"#);
}

#[test]
fn sum_follows_upstream_twelve_significant_digits() {
    let (merged, _, _) = run(
        r#"{"x":[0.1,"d"]}"#,
        r#"[{"op":"delta","path":"/x","value":0.2}]"#,
        &[],
    );
    assert_eq!(merged, r#"{"x":[0.3,"d"]}"#);
    let (merged, _, _) = run(
        r#"{"x":[123456789012.5,"d"]}"#,
        r#"[{"op":"delta","path":"/x","value":0.123}]"#,
        &[],
    );
    assert_eq!(merged, r#"{"x":[123456789013,"d"]}"#);
}

#[test]
fn replace_delta_replace_in_one_batch_stay_typed() {
    let (merged, records, _) = run(
        r#"{"HP":[50,"生命"]}"#,
        r#"[{"op":"replace","path":"/HP","value":60},{"op":"delta","path":"/HP","value":5}]"#,
        &[],
    );
    assert_eq!(merged, r#"{"HP":[65,"生命"]}"#);
    assert!(records.is_empty(), "{records:?}");
    let (merged, _, _) = run(
        r#"{"HP":[50,"生命"]}"#,
        r#"[{"op":"replace","path":"/HP","value":60},{"op":"delta","path":"/HP","value":5},{"op":"replace","path":"/HP","value":"70"}]"#,
        &[],
    );
    assert_eq!(merged, r#"{"HP":[70,"生命"]}"#);
}

#[test]
fn overflowing_sum_persists_as_null_like_upstream() {
    let (merged, records, _) = run(
        r#"{"x":[1e308,"d"]}"#,
        r#"[{"op":"delta","path":"/x","value":1e308}]"#,
        &[],
    );
    assert_eq!(merged, r#"{"x":[null,"d"]}"#);
    assert!(records.is_empty(), "{records:?}");
}

#[test]
fn non_finite_input_is_rejected_and_untouched() {
    let (merged, records, _) = run(
        r#"{"HP":[50,"生命"]}"#,
        r#"[{"op":"delta","path":"/HP","value":"NaN"}]"#,
        &[],
    );
    assert_eq!(merged, r#"{"HP":[50,"生命"]}"#);
    assert_eq!(records[0].kind, RecordKind::Rejected);
}

#[test]
fn explicit_range_clamps_both_ends_and_records_the_clamped_value() {
    for (step, expected, shown) in [
        (-80, r#"{"HP":[0,"生命"]}"#, "0"),
        (80, r#"{"HP":[100,"生命"]}"#, "100"),
    ] {
        let (merged, records, _) = run(
            r#"{"HP":[50,"生命"]}"#,
            &format!(r#"[{{"op":"delta","path":"/HP","value":{step}}}]"#),
            &[("HP", ranged(0.0, 100.0))],
        );
        assert_eq!(merged, expected);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].kind, RecordKind::Clamped);
        assert_eq!(
            records[0].detail,
            format!("HP 已夾在範圍內，目前值 {shown}。")
        );
    }
}

#[test]
fn index_paths_persist_like_upstream() {
    let (merged, records, _) = run(
        r#"{"HP":[50,"生命"]}"#,
        r#"[{"op":"replace","path":"/HP/0","value":55}]"#,
        &[],
    );
    assert!(records.is_empty(), "{records:?}");
    assert_eq!(merged, r#"{"HP":[55,"生命"]}"#);
    let (merged, records, _) = run(
        r#"{"物品":["劍","盾","弓"]}"#,
        r#"[{"op":"replace","path":"/物品/1","value":"斧"}]"#,
        &[],
    );
    assert!(records.is_empty(), "{records:?}");
    assert_eq!(merged, r#"{"物品":["劍","斧","弓"]}"#);
}

/// 恰好兩個字串的清單：索引路徑照一般陣列；整列 replace 寫陣列整個換掉（偏離上游的巢狀），寫非陣列仍只改第 0 項。
#[test]
fn two_string_list_whole_replace_with_an_array_replaces_it() {
    let stat = r#"{"背包":["劍","盾"]}"#;
    let (merged, _, _) = run(
        stat,
        r#"[{"op":"replace","path":"/背包/1","value":"弓"}]"#,
        &[],
    );
    assert_eq!(merged, r#"{"背包":["劍","弓"]}"#);
    let (merged, _, _) = run(
        stat,
        r#"[{"op":"replace","path":"/背包","value":["劍","盾","弓"]}]"#,
        &[],
    );
    assert_eq!(merged, r#"{"背包":["劍","盾","弓"]}"#);
    let (merged, _, _) = run(
        stat,
        r#"[{"op":"replace","path":"/背包","value":"弓"}]"#,
        &[],
    );
    assert_eq!(merged, r#"{"背包":["弓","盾"]}"#);
}

/// mvu-array-index-path：索引路徑只轉換被操作的元素，其餘元素保留原始 JSON（大整數不經 f64；墊片的 JS 表示不了，
/// 只在後端測）。
#[test]
fn index_paths_keep_untouched_elements_verbatim() {
    let (merged, records, _) = run(
        r#"{"A":[9007199254740993,1,{"n":12345678901234567890}]}"#,
        r#"[{"op":"replace","path":"/A/1","value":2},{"op":"insert","path":"/A/0","value":0},{"op":"remove","path":"/A/2"}]"#,
        &[],
    );
    assert!(records.is_empty(), "{records:?}");
    assert_eq!(
        merged,
        r#"{"A":[0,9007199254740993,{"n":12345678901234567890}]}"#
    );
}

/// Upstream 桌 `insert /物件/-`：樹落成實際的鍵（"0"），視圖照同一個鍵寫，同批 replace 後落檔沒有重複鍵。
#[test]
fn insert_dash_into_an_object_writes_the_actual_key() {
    let (merged, records, changes) = run(
        r#"{"物品":{}}"#,
        r#"[{"op":"insert","path":"/物品/-","value":"劍"},{"op":"replace","path":"/物品/0","value":"盾"}]"#,
        &[],
    );
    assert!(records.is_empty(), "{records:?}");
    assert_eq!(merged, r#"{"物品":{"0":"盾"}}"#);
    assert!(changes.contains_key("物品.0") && !changes.contains_key("物品.-"));
}
