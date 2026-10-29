use crate::commands::NEXT_TEMP_ID;
use crate::data;
use std::sync::atomic::Ordering;

/// 零額度讀出 GM lane 實際送出的提示詞（凍結 system＋回合尾段），不打 AI。手動執行：
/// `TT_PROMPT_ROOT=<資料根目錄> TT_PROMPT_WORLD=<桌 id> cargo test --lib dump_gm_lane_prompt -- --ignored --nocapture`
/// 會把 system 與回合尾段印到 stdout，並把兩段寫進 TT_PROMPT_OUT（有設才寫）。
#[test]
#[ignore]
fn dump_gm_lane_prompt() {
    let (Ok(root), Ok(world_id)) = (
        std::env::var("TT_PROMPT_ROOT"),
        std::env::var("TT_PROMPT_WORLD"),
    ) else {
        eprintln!("未設 TT_PROMPT_ROOT／TT_PROMPT_WORLD，略過");
        return;
    };
    let root = std::path::PathBuf::from(root);
    let materials = super::gm_materials(&root, &world_id).unwrap();
    let roster = crate::chat_assembly::speaker_roster(&materials.cards);
    let player_name = materials.player.as_ref().map(|card| card.name.as_str());
    let lang = "zh-TW";
    let scan = crate::chat_assembly::test_gm_scan(&root, &world_id, &materials, lang);
    let (instruction_message, closing) = super::gm_turn_instruction(
        &root,
        &world_id,
        &materials,
        &scan,
        &roster,
        player_name,
        lang,
    );
    let scope = crate::transport::state_scope(
        &materials.state.state,
        &materials.state.mechanism,
        &materials.cards,
        materials.player.as_ref(),
        &materials.state.branch_bindings,
        false,
        &[],
    );
    let system = crate::transport::gm_lane_system_with(
        &materials.cards,
        materials.player.as_ref(),
        &scan,
        &materials.state.mechanism,
        lang,
        materials.takeover,
        &Default::default(),
    );
    let instruction = format!("{}\n{closing}", instruction_message.content);
    let turn = crate::transport::gm_lane_turn(
        &scan,
        materials.player.as_ref(),
        &materials.state.state,
        &materials.state.mechanism,
        &scope,
        &instruction,
        lang,
    );
    let dump = format!(
        "===== SYSTEM =====\n{system}\n\n===== TURN TAIL =====\n{}\n",
        turn.tail
    );
    println!("{dump}");
    if let Ok(out) = std::env::var("TT_PROMPT_OUT") {
        std::fs::write(out, &dump).unwrap();
    }
}

/// mvu-replace-numeric：GM 回合在 gm_materials 算一次策略，提示詞與提交（照 turn.commit 的做法把同一份
/// 帶進鎖內讀的狀態）都用它——沒重構的 MVU 桌兩邊都放行 replace，重構後兩邊都回原規則。
#[test]
fn gm_turn_uses_one_numeric_policy_for_prompt_and_commit() {
    let root = std::env::temp_dir().join(format!(
        "table-tavern-gm-numeric-{}-{}",
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let world_id = data::create_world(&root, "測試桌").unwrap();
    let raw = serde_json::json!({"data": {"name": "莉亞", "character_book": {"entries": [
        {"comment": "[initvar]初始", "enabled": false, "content": "World:\n  Gold: 100000"}
    ]}}})
    .to_string();
    crate::import::import_character(&root, &world_id, raw.as_bytes(), "#3366ff", "zh-TW").unwrap();
    let reply = "旁白<UpdateVariable><JSONPatch>[{\"op\":\"replace\",\"path\":\"/World/Gold\",\"value\":99000}]</JSONPatch></UpdateVariable>";
    let block = crate::transport::extract_state_block(reply);
    let run = |expected: data::NumericUpdate, expected_gold: &str, prompt: &str| {
        let materials = super::gm_materials(&root, &world_id).unwrap();
        assert_eq!(materials.state.mechanism.numeric_update, expected);
        let scan = crate::chat_assembly::test_gm_scan(&root, &world_id, &materials, "zh-TW");
        let system = crate::transport::gm_lane_system_with(
            &materials.cards,
            materials.player.as_ref(),
            &scan,
            &materials.state.mechanism,
            "zh-TW",
            materials.takeover,
            &Default::default(),
        );
        assert!(system.contains(prompt), "{system}");
        let mut state = data::read_state(&root, &world_id).unwrap();
        state.mechanism.numeric_update = materials.state.mechanism.numeric_update;
        crate::mechanism::apply_block(&mut state, &block, "阿濤");
        let gold =
            data::node_at(&state.state.tree, &["World".to_owned(), "Gold".to_owned()]).cloned();
        assert_eq!(gold, Some(data::StateNode::Leaf(expected_gold.to_owned())));
    };
    run(
        data::NumericUpdate::Upstream,
        "99000",
        "也可以用 replace 直接寫新值",
    );
    let mut state = data::read_state(&root, &world_id).unwrap();
    state.refactor_mode = Some("interface".to_owned());
    data::write_state(&root, &world_id, &state).unwrap();
    run(
        data::NumericUpdate::DeltaOnly,
        "100000",
        "給絕對值會被系統擋下",
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// mvu-replace-numeric × gm-format-directive-missing-target：沒重構的 MVU 卡、原卡格式條目照原卡啟用時，
/// 完整 GM 提示（凍結 system＋回合尾段的導演指示與收尾句）裡，上游版協定、卡的格式條目、導演指示三者
/// 不能互相矛盾：不能再出現「數字欄只收 delta／絕對值會被擋／上下限由系統把關」，指示也不能禁掉更新區塊。
#[test]
fn full_gm_prompt_with_card_format_and_upstream_protocol_is_consistent() {
    for (label, format_tag) in [("named", "<StatusBlock>"), ("neutral", "【狀態】")] {
        let root = std::env::temp_dir().join(format!(
            "table-tavern-gm-format-upstream-{label}-{}-{}",
            std::process::id(),
            NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let world_id = data::create_world(&root, "測試桌").unwrap();
        let format = format!(
                "每回合正文後輸出 {format_tag} 狀態欄，最後用 <UpdateVariable><JSONPatch> 回報變數，\
                 數值可直接 replace，例 {{\"op\":\"replace\",\"path\":\"/World/Gold\",\"value\":90}}。"
            );
        let raw = serde_json::json!({"data": {
                "name": "莉亞",
                "extensions": {"regex_scripts": [{
                    "scriptName": "狀態欄", "findRegex": "/<StatusBlock>([\\s\\S]*?)<\\/StatusBlock>/s",
                    "replaceString": "<div>$1</div>", "placement": [2], "disabled": false
                }]},
                "character_book": {"entries": [
                    {"comment": "[initvar]初始", "enabled": false, "content": "World:\n  Gold: 100"},
                    {"comment": "[mvu_update]变量更新规则", "enabled": true,
                     "content": "规则:\n  World:\n    Gold:\n      type: number\n      range: 0-1000\n"},
                    {"comment": "[mvu_update]变量输出格式", "enabled": true, "constant": true, "content": format}
                ]}
            }})
            .to_string();
        crate::import::import_character(&root, &world_id, raw.as_bytes(), "#3366ff", "zh-TW")
            .unwrap();
        let materials = super::gm_materials(&root, &world_id).unwrap();
        assert_eq!(
            materials.state.mechanism.numeric_update,
            data::NumericUpdate::Upstream,
            "{label}"
        );
        let lang = "zh-TW";
        let scan = crate::chat_assembly::test_gm_scan(&root, &world_id, &materials, lang);
        let (instruction, closing) =
            super::gm_turn_instruction(&root, &world_id, &materials, &scan, &[], None, lang);
        let system = crate::transport::gm_lane_system_with(
            &materials.cards,
            materials.player.as_ref(),
            &scan,
            &materials.state.mechanism,
            lang,
            materials.takeover,
            &Default::default(),
        );
        let prompt = format!("{system}\n{}\n{closing}", instruction.content);
        // 卡的格式條目照原卡啟用、全文進提示；上游版協定在
        assert!(prompt.contains("數值可直接 replace"), "{label}: {prompt}");
        assert!(prompt.contains("也可以用 replace 直接寫新值"), "{label}");
        assert!(prompt.contains("系統不會幫你夾在上下限內"), "{label}");
        for contradiction in [
            "數字欄一律用 delta",
            "給絕對值會被系統擋下",
            "只有上限改變（升級）才用 replace",
            "上下限與拒收由系統把關",
            "你只要說「這一幕變動了多少」",
        ] {
            assert!(!prompt.contains(contradiction), "{label}: {contradiction}");
        }
        match label {
            "named" => assert!(
                instruction.content.contains("[mvu_update]变量输出格式"),
                "{label}: {}",
                instruction.content
            ),
            _ => assert!(
                instruction.content.contains("狀態更新協定"),
                "{label}: {}",
                instruction.content
            ),
        }
        // 導演指示與收尾句不禁更新區塊（中性版不寫「只輸出正文」）
        for banned in ["只輸出正文", "不要輸出更新區塊", "不要輸出狀態更新"] {
            assert!(!instruction.content.contains(banned), "{label}: {banned}");
            assert!(!closing.contains(banned), "{label}: {banned}");
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
