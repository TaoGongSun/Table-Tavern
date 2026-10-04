//! mvu-replace-numeric：跟前端墊片（card-mvu-parse-source.test.ts）跑同一份案例。照 GM 回合提交
//! （write.rs `apply_gm_block`）的路：stat_data 投影成樹 → apply_block_typed（Upstream 策略、帶 stat_data）
//! → 照帶型別寫值合回 stat_data。
use super::convert::{merge_tree_change_typed, stat_to_tree, NewValues};
use super::json::{parse, Json};
use crate::data::NumericUpdate;
use std::collections::BTreeMap;

const CASES: &str = include_str!("../../../../src/shared/contracts/mvu-replace-parity.json");

fn world(tree: super::convert::Tree) -> crate::data::WorldState {
    let mut state: crate::data::WorldState = serde_json::from_value(serde_json::json!({
        "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        "name": "比對桌",
        "current_scene": 0
    }))
    .unwrap();
    state.state.tree = tree;
    state.mechanism.numeric_update = NumericUpdate::Upstream;
    state
}

#[test]
fn upstream_replace_matches_the_shim_cases() {
    let fixture: serde_json::Value = serde_json::from_str(CASES).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let stat = parse(&case["stat_data"].to_string()).unwrap();
        let expected = parse(&case["expected"].to_string()).unwrap();
        let mut state = world(stat_to_tree(Some(&stat)));
        let before = state.state.tree.clone();
        let reply = format!(
            "旁白<UpdateVariable><JSONPatch>{}</JSONPatch></UpdateVariable>",
            case["patch"]
        );
        let outcome = crate::mechanism::apply_block_typed(
            &mut state,
            &crate::transport::extract_state_block(&reply),
            "阿濤",
            Some(&stat),
        );
        // 墊片沒有記帳：後端該拒收的路徑由案例標明（`backend_rejected`），其餘不得拒收
        let rejected: Vec<&str> = outcome
            .records
            .iter()
            .filter(|record| record.kind == crate::mechanism::RecordKind::Rejected)
            .map(|record| record.path.as_str())
            .collect();
        let expected_rejected: Vec<&str> = case["backend_rejected"]
            .as_array()
            .map(|paths| paths.iter().filter_map(|path| path.as_str()).collect())
            .unwrap_or_default();
        assert_eq!(rejected, expected_rejected, "{name}: {:?}", outcome.records);
        let types = BTreeMap::new();
        let rules = NewValues {
            types: &types,
            keep_strings: false,
            clean: None,
        };
        let merged: Json = merge_tree_change_typed(
            &stat,
            &before,
            &state.state.tree,
            &rules,
            outcome.typed.as_ref(),
        );
        assert_eq!(merged.to_text(), expected.to_text(), "{name}");
    }
}

/// move（墊片照上游不套用 move，只能後端單測）：只有明確搬成功才照原型別寫回目的欄；拒收（來源或目的是底線
/// 唯讀欄）、失敗（目的中間層被葉子占住）都不動帶型別視圖。
#[test]
fn move_keeps_types_only_when_it_actually_moves() {
    let run = |stat: &str, from: &str, to: &str| -> (String, Vec<crate::mechanism::Record>) {
        let stat = parse(stat).unwrap();
        let mut state = world(stat_to_tree(Some(&stat)));
        let before = state.state.tree.clone();
        let reply = format!(
            "<UpdateVariable><JSONPatch>[{{\"op\":\"move\",\"from\":\"{from}\",\"to\":\"{to}\"}}]</JSONPatch></UpdateVariable>"
        );
        let outcome = crate::mechanism::apply_block_typed(
            &mut state,
            &crate::transport::extract_state_block(&reply),
            "阿濤",
            Some(&stat),
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
            &state.state.tree,
            &rules,
            outcome.typed.as_ref(),
        );
        (merged.to_text(), outcome.records)
    };
    let kinds = |records: &[crate::mechanism::Record]| -> Vec<crate::mechanism::RecordKind> {
        records.iter().map(|record| record.kind).collect()
    };
    use crate::mechanism::RecordKind::{Error, Rejected};
    // 拒收：來源唯讀、目的唯讀
    let (merged, records) = run(r#"{"_src":"123","dst":123}"#, "/_src", "/dst");
    assert_eq!(
        (merged.as_str(), kinds(&records)),
        (r#"{"_src":"123","dst":123}"#, vec![Rejected])
    );
    let (merged, records) = run(r#"{"src":"123","_dst":123}"#, "/src", "/_dst");
    assert_eq!(
        (merged.as_str(), kinds(&records)),
        (r#"{"src":"123","_dst":123}"#, vec![Rejected])
    );
    // 失敗：目的中間層是葉子
    let (merged, records) = run(r#"{"src":"123","blk":5}"#, "/src", "/blk/x");
    assert_eq!(
        (merged.as_str(), kinds(&records)),
        (r#"{"src":"123","blk":5}"#, vec![Error])
    );
    // 成功：字串 "123" 搬到原本是數字的欄，照原值型別寫字串
    let (merged, records) = run(r#"{"src":"123","dst":1}"#, "/src", "/dst");
    assert_eq!((merged.as_str(), records.len()), (r#"{"dst":"123"}"#, 0));
}

/// 變數模式真桌（載入 MVU 的卡，沒重構）：開場白與 GM 回合提交都把 stat_data 帶進上游 set，新表照原型別寫：
/// 字串欄寫數字字串仍是字串、數字欄寫轉不成數字的字串是 null、`[值, 說明]` 只改第 0 項、
/// 同一批先 null 再 "85" 得到字串 "85"。
#[test]
fn opening_and_gm_commit_keep_json_types_in_message_vars() {
    use super::tests::{append_part, begin, mvu_world, stat_text};
    use crate::data::state_commit::with_commit;
    use crate::data::test_support::TestRoot;

    let root = TestRoot::new("mvu-typed-set");
    let world_id = mvu_world(&root);
    let raw = "開場白<UpdateVariable><JSONPatch>[\
        {\"op\":\"replace\",\"path\":\"/角色/名字\",\"value\":\"123\"},\
        {\"op\":\"replace\",\"path\":\"/金錢\",\"value\":\"很多\"}]</JSONPatch></UpdateVariable>";
    let (opening, outcome) = crate::data::append_opening(
        root.path(),
        &world_id,
        0,
        "opening",
        raw,
        &crate::transport::extract_state_block(raw),
        "阿濤",
    )
    .unwrap();
    assert!(outcome.records.is_empty(), "{:?}", outcome.records);
    assert_eq!(
        stat_text(&opening),
        r#"{"角色":{"名字":"123","好感":10,"標籤":[1,"說明"]},"金錢":null}"#
    );

    begin(root.path(), &world_id, "t1");
    let reply = "旁白<UpdateVariable><JSONPatch>[\
        {\"op\":\"replace\",\"path\":\"/角色/標籤\",\"value\":false},\
        {\"op\":\"replace\",\"path\":\"/角色/好感\",\"value\":null},\
        {\"op\":\"replace\",\"path\":\"/角色/好感\",\"value\":\"85\"}]</JSONPatch></UpdateVariable>";
    let block = crate::transport::extract_state_block(reply);
    with_commit(root.path(), &world_id, |tx| {
        let main = super::PendingMain {
            text: "GM".to_owned(),
            raw: None,
            truncated: false,
        };
        super::apply_gm_block(
            tx,
            "t1",
            main,
            |state, stat| {
                state.mechanism.numeric_update =
                    crate::mechanism::resolve_numeric_update(tx.root, tx.world_id, state);
                let outcome = crate::mechanism::apply_block_typed(state, &block, "阿濤", stat);
                let typed = outcome.typed.clone();
                (outcome, typed)
            },
            |_, _| Vec::new(),
        )
    })
    .unwrap()
    .expect("提交成功");
    let main = append_part(root.path(), &world_id, "t1", super::PART_MAIN, "GM").unwrap();
    let stat = stat_text(&main);
    assert_eq!(
        stat,
        r#"{"角色":{"名字":"123","好感":"85","標籤":[0,"說明"]},"金錢":null}"#
    );
}
