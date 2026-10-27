# import/ 測試檔收進子資料夾

## Summary
`src-tauri/src/import/` 超過 docs/STRUCTURE.md 的資料夾檔數上限（約 20）。純搬檔：10 支獨立的 `*_tests.rs` 收進 `import/tests/`（`refactor/tests/` 同形），不改測試內容，只改 mod 宣告與 `super::` → `crate::import::` 的 use 路徑。

## Progress
- 搬完：book_shape、card_book、card_image、card_view、clean_cards、foreign_ids、initvar_list、ledger_route、mvu_replace、widened_entries。`import/` 的 .rs 從 24 降到 14。
- 測試收集數搬前後同為 1956（`import::` 底下 217），名稱只差模組前綴。
- 不搬：`test_support.rs`（被 refactor／web_save／各模組內測試用，是輔助不是測試）、`card_tests.rs`（以 `#[path]` 掛成 `card.rs` 的子模組，用 `use super::*` 測私有項，搬走會斷）。等以後 `card.rs` 改成 `card/` 資料夾時再把它收成 `card/tests.rs`。

## Next action
送審（Sol、Grok、opus-review）後結案：squash 合進 main、刪分支。
