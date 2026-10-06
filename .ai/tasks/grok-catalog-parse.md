# 設定頁 grok 模型下拉只列得出預設模型

Status: todo

## Summary
`src-tauri/src/cli/catalog.rs` 的 `parse_grok_catalog` 只認 `*` 開頭的列。grok 1.0.46 的 `grok models` 只有預設模型用 `*`，其他改用 `-`（例：`* grok-4.7 (default)`、`- grok-4.6`），所以設定頁 grok 下拉只剩 grok-4.7，4.6／4.5／4.7-build-fast 選不到（只能走「自訂」手填）。2026-10-06 grok-system-override 實測時發現。

## Next action
`*` 與 `-` 都收、只認縮排的模型列（別把說明文字當模型），補 1.0.46 實際輸出的單元測試。
