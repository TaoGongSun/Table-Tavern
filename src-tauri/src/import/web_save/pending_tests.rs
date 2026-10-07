//! 匯入成功之後的未確認記錄：放棄時照記錄撤回跨桌層再刪桌、確認後記錄清掉、記錄先於跨桌層落地。
use super::tests::{bytes, fixture, layer_json, legit_write, seed_layer, world_ids};
use super::*;
use crate::data::card_vars::read_layer;
use crate::import::test_support::TestRoot;
use serde_json::json;

const LANG: &str = "zh-TW";

fn pending_json(root: &TestRoot, world_id: &str) -> Option<Value> {
    let path = data::web_save_pending_path(root.path(), world_id).unwrap();
    std::fs::read(path)
        .ok()
        .map(|text| serde_json::from_slice(&text).unwrap())
}

/// 別的桌先有 `web_global_shared`，匯入 short.json 就只補 `web_global_new` 與 extension 的 `theme`。
fn imported_short(root: &TestRoot) -> (String, String) {
    let other = data::create_world(root.path(), "原有的桌").unwrap();
    seed_layer(
        root,
        &other,
        Layer::Global,
        None,
        r#"{"web_global_shared":"desktop-value"}"#,
    );
    let imported = import_web_save(root.path(), &bytes(&fixture("short.json")), LANG).unwrap();
    (other, imported.world_id)
}

/// 具名反例：跨桌補缺成功→（卡片 storage 寫不進去）玩家放棄→撤回跨桌層、刪新桌。期間的合法寫入保留：
/// 沒被匯入碰過的層照留；被補過、之後又被寫過（rev 變了）的層整層不撤，沒撤回的鍵連同原因回報。
#[test]
fn abandon_after_shared_fill_retracts_shared_layers_and_deletes_the_table() {
    let root = TestRoot::new("web-save-abandon");
    let (other, w) = imported_short(&root);
    let journal = pending_json(&root, &w).expect("匯入成功、還沒確認：記錄在");
    assert_eq!(journal["shared_fill"].as_array().unwrap().len(), 2);
    // 匯入之後、放棄之前：別桌合法寫了 preset（匯入沒碰）與 extension（匯入補過）
    legit_write(&root, &other, Layer::Preset, None, |table| {
        table["concurrent"] = json!("yes")
    });
    legit_write(
        &root,
        &other,
        Layer::Extension,
        Some("ext.example"),
        |table| table["mine"] = json!(1),
    );

    let error = discard_web_save_import(root.path(), &w, "STORAGE_FAILED")
        .unwrap_err()
        .to_string();
    assert!(error.contains("web_save_cleanup_incomplete"), "{error}");
    assert!(error.contains("STORAGE_FAILED"), "回報帶放棄原因：{error}");
    assert!(
        error.contains("extension:ext.example:theme"),
        "rev 變了的層回報沒撤回的鍵：{error}"
    );
    assert!(
        !error.contains("global:"),
        "global 已撤回，不在殘留：{error}"
    );
    assert!(!error.contains("world:"), "新桌已刪：{error}");

    assert_eq!(world_ids(&root), vec![other.clone()], "新桌不存在");
    assert_eq!(
        layer_json(&root, &other, Layer::Global, None),
        json!({"web_global_shared": "desktop-value"}),
        "global 回到匯入前"
    );
    assert_eq!(
        layer_json(&root, &other, Layer::Preset, None),
        json!({"concurrent": "yes"})
    );
    assert_eq!(
        layer_json(&root, &other, Layer::Extension, Some("ext.example")),
        json!({"theme": "dark", "mine": 1}),
        "rev 變了的層整層不動"
    );
}

#[test]
fn abandon_without_other_writes_restores_every_shared_layer() {
    let root = TestRoot::new("web-save-abandon-clean");
    let (other, w) = imported_short(&root);
    discard_web_save_import(root.path(), &w, "STORAGE_FAILED").unwrap();
    assert_eq!(world_ids(&root), vec![other.clone()]);
    assert_eq!(
        layer_json(&root, &other, Layer::Global, None),
        json!({"web_global_shared": "desktop-value"})
    );
    assert_eq!(
        read_layer(root.path(), &other, Layer::Extension, Some("ext.example"))
            .unwrap()
            .rev,
        None,
        "匯入前沒有檔的層撤完回到不存在"
    );
}

/// 確認完成：記錄清掉，補的鍵從此算玩家的；之後再走放棄路徑不撤回、不刪桌，回報殘桌。
#[test]
fn confirming_the_import_clears_the_journal() {
    let root = TestRoot::new("web-save-confirm");
    let (other, w) = imported_short(&root);
    confirm_web_save_import(root.path(), &w).unwrap();
    assert_eq!(pending_json(&root, &w), None, "記錄清掉");
    confirm_web_save_import(root.path(), &w).unwrap();

    let error = discard_web_save_import(root.path(), &w, "STORAGE_FAILED")
        .unwrap_err()
        .to_string();
    assert!(error.contains("web_save_cleanup_incomplete"), "{error}");
    assert!(error.contains(&format!("world:{w}")), "{error}");
    assert!(world_ids(&root).contains(&w), "確認過的桌不刪");
    assert_eq!(
        layer_json(&root, &other, Layer::Global, None),
        json!({"web_global_shared": "desktop-value", "web_global_new": "from-web"})
    );
}

/// 每一層跨桌層真的寫下去之前，記錄已經含這一層（崩潰在任何一步之後，記錄都涵蓋已落地的補缺）。
#[test]
fn the_journal_lands_before_each_shared_layer_write() {
    let root = TestRoot::new("web-save-journal-first");
    let mut seen = Vec::new();
    let imported = import_with(
        root.path(),
        &bytes(&fixture("short.json")),
        LANG,
        &mut |count, filled| {
            let worlds = data::list_worlds(root.path()).unwrap();
            let journal = pending_json(&root, &worlds[0].id).unwrap();
            let entries = journal["shared_fill"].as_array().unwrap();
            assert_eq!(entries.len(), count);
            assert_eq!(
                entries[count - 1]["rev"],
                json!(filled.rev.clone().unwrap())
            );
            seen.push(filled.layer);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(seen, vec![Layer::Global, Layer::Extension]);
    assert!(pending_json(&root, &imported.world_id).is_some());
}

/// 記錄寫不進去：那一層不寫，匯入失敗、整桌收掉，跨桌層原封不動。
#[test]
fn a_journal_write_failure_keeps_the_shared_layer_untouched() {
    let root = TestRoot::new("web-save-journal-fail");
    let other = data::create_world(root.path(), "原有的桌").unwrap();
    seed_layer(
        &root,
        &other,
        Layer::Global,
        None,
        r#"{"web_global_shared":"desktop-value"}"#,
    );
    let before = read_layer(root.path(), &other, Layer::Global, None).unwrap();
    {
        // 第一次是建桌後的空記錄，第二次是補 global 之前的記錄
        let _guard = data::WriteFailGuard::partial_ending_after("web-save-pending.json.tmp", 1, 1);
        let error = import_web_save(root.path(), &bytes(&fixture("short.json")), LANG)
            .unwrap_err()
            .to_string();
        assert!(!error.contains("web_save_cleanup_incomplete"), "{error}");
    }
    assert_eq!(world_ids(&root), vec![other.clone()]);
    assert_eq!(
        read_layer(root.path(), &other, Layer::Global, None).unwrap(),
        before
    );
}

/// 記錄已寫、層沒寫成（崩潰或寫入失敗）：撤回看到層還是補之前的樣子就什麼都不做、也不回報。
#[test]
fn retracting_a_journaled_fill_that_never_landed_is_a_no_op() {
    let root = TestRoot::new("web-save-never-landed");
    let w = data::create_world(root.path(), "桌").unwrap();
    seed_layer(&root, &w, Layer::Global, None, r#"{"old":0}"#);
    for (layer, id) in [(Layer::Global, None), (Layer::Extension, Some("x"))] {
        let mut journaled = None;
        let result = card_vars::fill_missing(
            root.path(),
            &w,
            layer,
            id,
            &message_vars::parse_table(r#"{"new":1}"#).unwrap(),
            &mut |filled| {
                journaled = Some(filled.clone());
                Err(data::invalid_data("crash after the journal"))
            },
        );
        assert!(result.is_err());
        let journaled = journaled.expect("記錄有寫");
        assert_eq!(
            card_vars::retract_filled(root.path(), &w, &journaled).unwrap(),
            Vec::<String>::new()
        );
    }
    assert_eq!(
        layer_json(&root, &w, Layer::Global, None),
        json!({"old": 0})
    );
    assert_eq!(
        read_layer(root.path(), &w, Layer::Extension, Some("x"))
            .unwrap()
            .rev,
        None
    );
}
