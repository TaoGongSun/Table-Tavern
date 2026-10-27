fn main() {
    reject_harness_config_without_feature();
    require_mvu_bundle();
    tauri_build::build()
}

/// 桌面 MVU 引擎的 JS bundle（mvu_engine/runtime.rs 以 include_str! 嵌入）由 npm 產生、不進版控：缺了就停下說要跑哪個指令。
fn require_mvu_bundle() {
    println!("cargo:rerun-if-changed=gen/mvu-engine.js");
    if !std::path::Path::new("gen/mvu-engine.js").exists() {
        panic!("缺 src-tauri/gen/mvu-engine.js：先在 repo 根目錄跑 npm run build:mvu-bundle（npm run build 也會產生）");
    }
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
