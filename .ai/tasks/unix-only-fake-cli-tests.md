# unix-only-fake-cli-tests — 123 個測試用 shell 腳本假 CLI，Windows 跑不到

## Summary
windows-ci-red（2026-10-28）查出：`src-tauri/src` 有 123 個測試限 `#[cfg(unix)]`，分布在 40 個檔，原因是假 CLI 用 shell 腳本寫。Windows CI 因此測不到 lane、runner 這些跨平台邏輯。業界做法是用 Rust 寫測試用執行檔（`[[bin]]`＋`CARGO_BIN_EXE_`）或 assert_cmd，做跨平台的假 CLI。

## Next action
未排程。先盤點各測試的假 CLI 實際需要哪些行為，評估改成 Rust 假 CLI 的工作量。
