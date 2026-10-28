fn main() {
    reject_harness_config_without_feature();
    require_mvu_bundle();
    if embed_windows_manifest() {
        let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
        tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
            .expect("tauri-build 失敗");
    } else {
        tauri_build::build()
    }
}

/// Windows MSVC：tauri-build 的應用程式資訊清單（Common Controls v6）只經 `rustc-link-arg-bins`
/// 連進 app 執行檔，`cargo test` 的測試程式拿不到，一啟動就 STATUS_ENTRYPOINT_NOT_FOUND。
/// 改由連結器把同一份清單嵌進所有連結產物（app、測試程式），tauri-build 那份關掉免得重複。
/// 做法同 tauri 本身 build.rs 的 embed_manifest_for_tests。
fn embed_windows_manifest() -> bool {
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if !(windows && msvc) {
        return false;
    }
    let manifest =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    true
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
