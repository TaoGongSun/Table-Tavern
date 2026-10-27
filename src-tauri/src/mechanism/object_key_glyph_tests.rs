//! mvu-object-key-glyph：replace／insert 的物件值，巢狀鍵對回樹上的寫法（見 `value_keys`）。
//! 走帶 stat_data 的 `apply_block_typed`，樹與帶型別視圖都要對，下一批同步也不能把別的字形帶回來。
use super::test_support::{mechanism_with, world_with};
use super::{apply_block_typed, BlockScope, Outcome, RecordKind};
use crate::data::message_vars::{parse_json, Json};
use crate::data::{self, NumericUpdate, StateNode};
use crate::transport::extract_state_block;

const PEOPLE: &str = r#"{"人物":{"贺铮":{"行动":"巡逻","装备":"长刀","关系":{"铁三":"友"}},"铁三":{"行动":"打铁","装备":"铁锤","体力":80,"关系":{"贺铮":"友"}}}}"#;

fn pairs_of(json: &Json) -> Vec<(String, String)> {
    fn walk(node: &Json, path: String, out: &mut Vec<(String, String)>) {
        match node {
            Json::Object(entries) => {
                for (key, child) in entries {
                    let next = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    walk(child, next, out);
                }
            }
            other => out.push((path, data::message_vars::leaf_text(other))),
        }
    }
    let mut out = Vec::new();
    walk(json, String::new(), &mut out);
    out
}

/// 一桌沒重構的 MVU 卡：樹與 stat_data 同一份。
fn table(stat: &str) -> (data::WorldState, Json) {
    let base = parse_json(stat).expect("fixture");
    let pairs = pairs_of(&base);
    let refs: Vec<(&str, &str)> = pairs
        .iter()
        .map(|(p, v)| (p.as_str(), v.as_str()))
        .collect();
    let mut mechanism = mechanism_with(&[]);
    mechanism.numeric_update = NumericUpdate::Upstream;
    (world_with(&refs, mechanism), base)
}

fn turn(world: &mut data::WorldState, base: &Json, patches: serde_json::Value) -> (Outcome, Json) {
    let reply =
        format!("旁白。\n<UpdateVariable><JSONPatch>{patches}</JSONPatch></UpdateVariable>");
    let outcome = apply_block_typed(
        world,
        &extract_state_block(&reply),
        "阿濤",
        Some(base),
        BlockScope::Table,
    );
    let doc = outcome.typed.as_ref().expect("帶型別批次").doc.clone();
    (outcome, doc)
}

fn tree_keys(world: &data::WorldState, path: &str) -> Vec<String> {
    let segments: Vec<String> = path.split('.').map(str::to_owned).collect();
    match data::node_at(&world.state.tree, &segments) {
        Some(StateNode::Branch(children)) => children.keys().cloned().collect(),
        _ => Vec::new(),
    }
}

fn doc_keys(doc: &Json, path: &str) -> Vec<String> {
    let mut node = Some(doc);
    for segment in path.split('.') {
        node = node.and_then(|n| n.get(segment));
    }
    let mut keys: Vec<String> = node
        .and_then(Json::as_object)
        .map(|entries| entries.iter().map(|(k, _)| k.clone()).collect())
        .unwrap_or_default();
    keys.sort();
    keys
}

fn rejected(outcome: &Outcome) -> Vec<(String, String)> {
    outcome
        .records
        .iter()
        .filter(|record| record.kind == RecordKind::Rejected)
        .map(|record| (record.path.clone(), record.detail.clone()))
        .collect()
}

fn both(world: &data::WorldState, doc: &Json, path: &str) -> Vec<String> {
    let tree = tree_keys(world, path);
    assert_eq!(
        tree,
        doc_keys(doc, path),
        "樹與 stat_data 的 {path} 鍵不一致"
    );
    tree
}

/// 同層既有鍵寫成另一種字形：落回原鍵，值照新的寫；下一批同步不把繁體帶回來。
#[test]
fn replace_item_object_folds_back_to_existing_keys() {
    let (mut world, base) = table(PEOPLE);
    let (outcome, doc) = turn(
        &mut world,
        &base,
        serde_json::json!([{"op":"replace","path":"/人物/贺铮","value":{"行動":"守夜","裝備":"长刀","關係":{"鐵三":"友"}}}]),
    );
    assert!(rejected(&outcome).is_empty(), "{:?}", outcome.records);
    assert_eq!(both(&world, &doc, "人物.贺铮"), ["关系", "行动", "装备"]);
    assert_eq!(both(&world, &doc, "人物.贺铮.关系"), ["铁三"]);
    assert_eq!(
        doc.get("人物")
            .and_then(|p| p.get("贺铮"))
            .and_then(|h| h.get("行动")),
        Some(&Json::String("守夜".into()))
    );

    let (_, next) = turn(
        &mut world,
        &doc,
        serde_json::json!([{"op":"replace","path":"/人物/铁三/行动","value":"歇息"}]),
    );
    assert_eq!(both(&world, &next, "人物.贺铮"), ["关系", "行动", "装备"]);
}

/// 兩層物件：人名鍵與欄位鍵都對回去；未配對的新項目拿同層兄弟的鍵當候補；對不上的是合法新鍵照收。
#[test]
fn replace_two_levels_with_new_item_uses_sibling_keys() {
    let (mut world, base) = table(PEOPLE);
    let (outcome, doc) = turn(
        &mut world,
        &base,
        serde_json::json!([{"op":"replace","path":"/人物","value":{
            "賀錚":{"行動":"守夜","裝備":"长刀"},
            "鐵三":{"行动":"打铁","装备":"铁锤","體力":70},
            "周掌櫃":{"行動":"算账","裝備":"算盘","關係":{"鐵三":"熟客"},"傷勢":"无"}
        }}]),
    );
    assert!(rejected(&outcome).is_empty(), "{:?}", outcome.records);
    assert_eq!(both(&world, &doc, "人物"), ["周掌櫃", "贺铮", "铁三"]);
    assert_eq!(both(&world, &doc, "人物.铁三"), ["体力", "行动", "装备"]);
    assert_eq!(
        both(&world, &doc, "人物.周掌櫃"),
        ["傷勢", "关系", "行动", "装备"]
    );
    assert_eq!(both(&world, &doc, "人物.周掌櫃.关系"), ["铁三"]);
}

/// 節點自己沒有的欄位，同層兄弟有：照兄弟的寫法（每層候補）。insert 新項目同樣處理；
/// 同批後一筆看得到前一筆寫進去的鍵。
#[test]
fn sibling_keys_fill_in_for_missing_fields_and_batch_sees_prior_writes() {
    let (mut world, base) = table(PEOPLE);
    let (outcome, doc) = turn(
        &mut world,
        &base,
        serde_json::json!([
            {"op":"replace","path":"/人物/贺铮","value":{"行动":"守夜","装备":"长刀","體力":60}},
            {"op":"insert","path":"/人物/老周","value":{"行動":"算账","心緒":"平"}},
            {"op":"replace","path":"/人物/老周","value":{"行動":"睡觉","心绪":"倦"}}
        ]),
    );
    assert!(rejected(&outcome).is_empty(), "{:?}", outcome.records);
    assert_eq!(both(&world, &doc, "人物.贺铮"), ["体力", "行动", "装备"]);
    assert_eq!(both(&world, &doc, "人物.老周"), ["心緒", "行动"]);
}

/// 寫入物件裡兩個鍵只差字形：這一筆拒收、樹與 stat_data 不動，同批其他筆照常。
#[test]
fn written_twins_reject_only_that_patch() {
    let (mut world, base) = table(PEOPLE);
    let before = world.state.tree.clone();
    let (outcome, doc) = turn(
        &mut world,
        &base,
        serde_json::json!([
            {"op":"replace","path":"/人物/贺铮","value":{"行动":"巡逻","行動":"守夜"}},
            {"op":"insert","path":"/札记","value":{"筆記":"甲","笔记":"乙"}},
            {"op":"replace","path":"/人物/铁三/行动","value":"歇息"}
        ]),
    );
    let rejects = rejected(&outcome);
    assert_eq!(rejects.len(), 2, "{rejects:?}");
    assert_eq!(rejects[0].0, "人物.贺铮");
    assert!(
        rejects[0].1.contains("只差字形，是同一個欄位寫了兩次"),
        "{}",
        rejects[0].1
    );
    assert_eq!(rejects[1].0, "札记");
    assert_eq!(
        world.state.tree.get("人物").and_then(|p| match p {
            StateNode::Branch(c) => c.get("贺铮"),
            _ => None,
        }),
        match before.get("人物") {
            Some(StateNode::Branch(c)) => c.get("贺铮"),
            _ => None,
        }
    );
    assert!(world.state.tree.get("札记").is_none());
    assert_eq!(both(&world, &doc, "人物.贺铮"), ["关系", "行动", "装备"]);
    assert_eq!(
        doc.get("人物")
            .and_then(|p| p.get("铁三"))
            .and_then(|t| t.get("行动")),
        Some(&Json::String("歇息".into()))
    );
}

/// 舊值本來就有的雙鍵照原字精確回寫：放行。
#[test]
fn preexisting_twins_written_back_exactly_pass() {
    let (mut world, base) = table(r#"{"人物":{"贺铮":{"行动":"巡逻","行動":"守夜"}}}"#);
    let (outcome, doc) = turn(
        &mut world,
        &base,
        serde_json::json!([{"op":"replace","path":"/人物/贺铮","value":{"行动":"巡逻","行動":"站岗"}}]),
    );
    assert!(rejected(&outcome).is_empty(), "{:?}", outcome.records);
    assert_eq!(both(&world, &doc, "人物.贺铮"), ["行动", "行動"]);
}

/// 寫入鍵對不上任何原鍵、又同時像兩個以上：這一筆拒收；精確命中其一照收。
#[test]
fn ambiguous_key_rejects_but_exact_hit_passes() {
    let (mut world, base) = table(r#"{"人物":{"贺铮":{"装备":"长刀","裝備":"短刀"}}}"#);
    let (outcome, _) = turn(
        &mut world,
        &base,
        serde_json::json!([
            {"op":"replace","path":"/人物/贺铮","value":{"裝备":"弓"}},
            {"op":"replace","path":"/人物/贺铮","value":{"裝備":"弓"}}
        ]),
    );
    let rejects = rejected(&outcome);
    assert_eq!(rejects.len(), 1, "{rejects:?}");
    assert!(
        rejects[0].1.contains("對到好幾個只差字形的欄位"),
        "{}",
        rejects[0].1
    );
    assert_eq!(tree_keys(&world, "人物.贺铮"), ["裝備"]);
}

/// 重構桌（原規則）的物件 replace 一樣折。
#[test]
fn delta_only_table_also_folds() {
    let (mut world, base) = table(PEOPLE);
    world.mechanism.numeric_update = NumericUpdate::DeltaOnly;
    let outcome = apply_block_typed(
        &mut world,
        &extract_state_block(
            "旁白。\n<UpdateVariable><JSONPatch>[{\"op\":\"replace\",\"path\":\"/人物/贺铮\",\"value\":{\"行動\":\"守夜\",\"裝備\":\"长刀\"}}]</JSONPatch></UpdateVariable>",
        ),
        "阿濤",
        Some(&base),
        BlockScope::Table,
    );
    assert!(rejected(&outcome).is_empty(), "{:?}", outcome.records);
    assert_eq!(tree_keys(&world, "人物.贺铮"), ["行动", "装备"]);
}

/// 既有項目新增一個分支：下一層拿兄弟同名分支當候補（贺铮新增關係，對铁三的 关系.贺铮）。
#[test]
fn new_branch_on_existing_item_uses_sibling_branch_keys() {
    let (mut world, base) =
        table(r#"{"人物":{"贺铮":{"行动":"巡逻"},"铁三":{"行动":"打铁","关系":{"贺铮":"友"}}}}"#);
    let (outcome, doc) = turn(
        &mut world,
        &base,
        serde_json::json!([{"op":"replace","path":"/人物/贺铮","value":{"行动":"巡逻","關係":{"賀錚":"自己"}}}]),
    );
    assert!(rejected(&outcome).is_empty(), "{:?}", outcome.records);
    assert_eq!(both(&world, &doc, "人物.贺铮"), ["关系", "行动"]);
    assert_eq!(both(&world, &doc, "人物.贺铮.关系"), ["贺铮"]);
}

/// 陣列裡的物件另案：路徑進到陣列照原行為，只差字形的雙鍵不拒收、照寫。
#[test]
fn array_element_paths_keep_original_behavior() {
    let (mut world, base) = table(r#"{"背包":[{"名称":"刀"}]}"#);
    let (outcome, doc) = turn(
        &mut world,
        &base,
        serde_json::json!([{"op":"replace","path":"/背包/0","value":{"名称":"甲","名稱":"乙"}}]),
    );
    assert!(rejected(&outcome).is_empty(), "{:?}", outcome.records);
    let item = doc.get("背包").and_then(|bag| match bag {
        Json::Array(items) => items.first(),
        _ => None,
    });
    assert_eq!(
        item.and_then(|i| i.get("名稱")),
        Some(&Json::String("乙".into()))
    );
    assert_eq!(
        item.and_then(|i| i.get("名称")),
        Some(&Json::String("甲".into()))
    );
}
