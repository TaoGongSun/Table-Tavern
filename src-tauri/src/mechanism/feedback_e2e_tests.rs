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
    assert!(note.starts_with("神經狀態 是清單欄"));
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
