//! 測試用：找跨平台假 CLI（`examples/fake_cli.rs`，`cargo test` 會順便編）並組它的環境變數。

use std::path::PathBuf;

/// 測試執行檔在 `target/<profile>/deps/`，examples 在旁邊的 `target/<profile>/examples/`。
pub(crate) fn program() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let path = exe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples")
        .join(format!("fake_cli{}", std::env::consts::EXE_SUFFIX));
    assert!(
        path.exists(),
        "找不到假 CLI {}：要用 `cargo test`（不加 --lib）才會編 examples",
        path.display()
    );
    path
}

/// 選角色的環境變數（`claude`／`agy`／`script`）。
pub(crate) fn mode(mode: &str) -> (String, String) {
    ("TT_FAKE_CLI_MODE".to_owned(), mode.to_owned())
}

/// `script` 角色：逐行指令見 `examples/fake_cli.rs` 的 script()。
pub(crate) fn script(lines: &[&str]) -> Vec<(String, String)> {
    vec![
        mode("script"),
        ("TT_FAKE_CLI_SCRIPT".to_owned(), lines.join("\n")),
    ]
}
