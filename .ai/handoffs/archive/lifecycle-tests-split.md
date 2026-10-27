# lifecycle-tests-split — data/scene/lifecycle.rs 測試搬到子目錄

## Summary
`src-tauri/src/data/scene/lifecycle.rs` 在 revert-fork-auto-hidden 結案時已 979 行，其中測試模組佔六百多行。比照 `character/tests.rs` 把測試搬到 `lifecycle/tests.rs`，純搬檔、不改行為。

## Next action
未排程；下次有案要動 lifecycle.rs 時先做，或閒時單獨做。

## 驗收方向
測試收集數搬前後相同、verify 綠。
