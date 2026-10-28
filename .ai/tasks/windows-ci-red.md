# windows-ci-red — Windows verify CI 自 2026-10-03 起一直紅

## Summary
2026-10-28 codex-lane-resume 結案時發現：verify.yml（windows-latest）自 874c26dd 後沒綠過，main 最新也同樣紅。兩層問題：
1. cargo test 編譯失敗：`fake_claude` 是 `#[cfg(unix)]`，`lanes/tests/opening_raw.rs:19`、`owner.rs:58`、`owner.rs:1207` 沒同樣限制。
2. 限 unix 後可編譯，但測試程式一啟動就 `0xc0000139 STATUS_ENTRYPOINT_NOT_FOUND`（Tauri 測試程式在 Windows 缺 manifest 的已知問題），一個 Windows 測試都跑不到。
現況：Windows 只驗得到 `cargo check`，執行期行為靠實機佇列。

## Next action
未排程。先修 1（cfg），再查 2 的業界做法（測試用 manifest／embed-resource、或測試改不連 Tauri runtime）。
