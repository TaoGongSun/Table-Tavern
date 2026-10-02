fn main() {
    reject_harness_config_without_feature();
    tauri_build::build()
}

/// 用了測試包設定（Tauri CLI `--config` 經 TAURI_CONFIG 傳進來）卻沒開 `test-harness` feature，
/// 會產出名稱像測試包、卻讀寫正式資料路徑的 app：建置期直接擋。
fn reject_harness_config_without_feature() {
    println!("cargo:rerun-if-env-changed=TAURI_CONFIG");
    let Ok(config) = std::env::var("TAURI_CONFIG") else {
        return;
    };
    let harness_config =
        config.contains("com.tabletavern.app.harness") || config.contains("dist-harness");
    if harness_config && std::env::var_os("CARGO_FEATURE_TEST_HARNESS").is_none() {
        panic!("測試包設定必須搭配 test-harness feature：請用 npm run harness:build");
    }
}
