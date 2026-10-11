//! takeover-text-field-delta 端到端：清單欄被下 delta → 拒收進 `world.state.notes` →
//! 下一輪 API（assemble_gm_messages）與 CLI（gm_lane_turn）的 GM 尾段都帶回饋；改對之後回饋清空。
use super::apply_block;
use super::test_support::{mechanism_with, rule, world_with};
use crate::data::{FieldKind, NumericUpdate};
use crate::transport::test_support::legacy::{assemble_gm_messages, gm_lane_turn};
use crate::transport::{StateBlock, StateScope};

const NEURO: &str = "neuro|疲勞|20|低度疲勞\nneuro|壓力|35|中度壓力";

fn block(json: &str) -> StateBlock {
    StateBlock {
        fields: Vec::new(),
        updates: vec![format!("<JSONPatch>{json}</JSONPatch>")],
        display: String::new(),
    }
}

fn gm_tails(world: &crate::data::WorldState) -> (String, String) {
    let scope = StateScope::default();
    let api = assemble_gm_messages(
        "",
        &[],
        None,
        &[],
        &[],
        &world.state,
        &world.mechanism,
        &scope,
        "zh-TW",
    );
    let cli = gm_lane_turn(
        &[],
        &[],
        None,
        &world.state,
        &world.mechanism,
        &scope,
        "（導演指示）",
        "zh-TW",
    );
    (api.last().unwrap().content.clone(), cli.tail)
}

#[test]
fn list_delta_rejection_reaches_next_gm_turn_on_both_transports_then_clears() {
    let mut mechanism = mechanism_with(&[("神經狀態", rule(FieldKind::List, None, None))]);
    mechanism.incremental = true;
    mechanism.numeric_update = NumericUpdate::DeltaOnly;
    let mut world = world_with(&[("神經狀態", NEURO)], mechanism);

    apply_block(
        &mut world,
        &block(r#"[{"op":"delta","path":"/神經狀態","value":"neuro|壓力|10|升高"}]"#),
        "阿濤",
    );
    assert_eq!(world.state.notes.len(), 1);
    let note = world.state.notes[0].clone();
    assert!(note.starts_with("/神經狀態 是清單欄"));
    for tail in [gm_tails(&world).0, gm_tails(&world).1] {
        assert!(tail.contains("## 上一輪被系統擋下的更新"), "{tail}");
        assert!(tail.contains(&note), "{tail}");
    }

    let fixed = "neuro|疲勞|20|低度疲勞\nneuro|壓力|45|高度壓力";
    apply_block(
        &mut world,
        &block(&format!(
            r#"[{{"op":"replace","path":"/神經狀態","value":{}}}]"#,
            serde_json::to_string(fixed).unwrap()
        )),
        "阿濤",
    );
    assert!(world.state.notes.is_empty());
    // 整串 replace 收下：未變的疲勞行原樣保留
    assert_eq!(
        crate::mechanism::tree::leaf_at(&world.state.tree, "神經狀態").as_deref(),
        Some(fixed)
    );
    let (api, cli) = gm_tails(&world);
    assert!(!api.contains("上一輪被系統擋下"));
    assert!(!cli.contains("上一輪被系統擋下"));
    assert!(cli.contains("neuro|壓力|45|高度壓力"));
}

/// mechanism-error-feedback：路徑寫錯在全量／增量桌都經 notes 帶到下一輪 API 與 CLI 的 GM 尾段，改對後清空。
#[test]
fn missing_path_feedback_reaches_both_transports_on_full_and_incremental_tables() {
    for incremental in [false, true] {
        let mut mechanism = mechanism_with(&[("角色.HP", rule(FieldKind::Number, None, None))]);
        mechanism.incremental = incremental;
        let mut world = world_with(&[("角色.HP", "10"), ("角色.MP", "3")], mechanism);

        apply_block(
            &mut world,
            &block(r#"[{"op":"delta","path":"/角色/生命","value":-2}]"#),
            "阿濤",
        );
        let note = "路徑不存在：/角色/生命。上一層現有：HP、MP。若要新增請用 insert。";
        assert_eq!(
            world.state.notes,
            vec![note.to_owned()],
            "incremental={incremental}"
        );
        for tail in [gm_tails(&world).0, gm_tails(&world).1] {
            assert!(
                tail.contains("## 上一輪被系統擋下的更新") && tail.contains(note),
                "{tail}"
            );
        }

        apply_block(
            &mut world,
            &block(r#"[{"op":"delta","path":"/角色/HP","value":-2}]"#),
            "阿濤",
        );
        assert!(world.state.notes.is_empty());
        let (api, cli) = gm_tails(&world);
        assert!(!api.contains("上一輪被系統擋下") && !cli.contains("上一輪被系統擋下"));
    }
}

/// 回饋照區塊原順序：解析拒收夾在兩筆套用錯誤之間也照位置排；跨區塊照區塊先後。
#[test]
fn notes_follow_source_order_across_parse_and_apply_and_blocks() {
    let mechanism = mechanism_with(&[]);
    let mut world = world_with(&[("A.x", "1"), ("B.y", "2")], mechanism);
    let blocks = StateBlock {
        fields: Vec::new(),
        updates: vec![
            r#"<JSONPatch>[{"op":"replace","path":"/A/z","value":1},{"op":"set","path":"/A/x","value":1},{"op":"remove","path":"/B/w"}]</JSONPatch>"#.to_owned(),
            r#"<JSONPatch>[{"op":"replace","path":"/C","value":1}]</JSONPatch>"#.to_owned(),
        ],
        display: String::new(),
    };
    apply_block(&mut world, &blocks, "阿濤");
    let heads: Vec<String> = world
        .state
        .notes
        .iter()
        .map(|note| note.chars().take(14).collect())
        .collect();
    assert_eq!(
        heads,
        vec![
            "路徑不存在：/A/z。上一層",
            "op「set」無法辨識（pa",
            "路徑不存在：/B/w。上一層",
            "路徑不存在：/C。頂層現有："
        ]
    );
}

/// `[]` 是合法的「本輪沒有變動」：不記錄、照常清掉上一輪的回饋。
#[test]
fn empty_patch_array_clears_previous_notes() {
    let mut world = world_with(&[("A", "1")], mechanism_with(&[]));
    world.state.notes = vec!["上一輪的回饋".to_owned()];
    let outcome = apply_block(&mut world, &block("[]"), "阿濤");
    assert!(outcome.records.is_empty());
    assert!(world.state.notes.is_empty());
}

/// 帳本壞值與衍生公式算不出來維持 Error，不進回饋。
#[test]
fn ledger_and_formula_errors_stay_out_of_notes() {
    let mut mechanism = mechanism_with(&[
        ("HP", rule(FieldKind::Number, None, None)),
        (
            "戰力",
            crate::mechanism::test_support::derived_rule("沒有這欄 * 2"),
        ),
    ]);
    mechanism.incremental = true;
    let mut world = world_with(&[("HP", "滿"), ("戰力", "0")], mechanism);
    let outcome = apply_block(
        &mut world,
        &block(r#"[{"op":"delta","path":"/HP","value":1}]"#),
        "阿濤",
    );
    let errors: Vec<&str> = outcome
        .records
        .iter()
        .filter(|record| record.kind == crate::mechanism::RecordKind::Error)
        .map(|record| record.path.as_str())
        .collect();
    assert_eq!(errors, vec!["HP", "戰力"]);
    assert!(world.state.notes.is_empty(), "{:?}", outcome.records);
}
