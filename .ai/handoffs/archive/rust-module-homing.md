# rust-module-homing — Rust 根層模組歸位（已結案）

落點表見 [plans/rust-module-homing.md](../../plans/rust-module-homing.md)。

## 結果

- 12 支有單一 owner 的根層檔搬進 `transport/`、`usage/`、`lanes/`、`cli/`、`import/`、`refactor_ai/`；`lanes`、`receipts` 的測試拆成 `tests.rs`。根層留 `inflight`／`ui_msg`／`openrouter_oauth`／`receipts` 與 2018 式模組根。
- 落點由模型依讀取方便決定〔作者裁決 2026-10-02〕；計畫與實作經 Sol 簽收（Grok 額度暫停）。
- 搬前後 `cargo test -- --list` 706 筆依前綴映射一致；verify 八步全綠（cargo test 705 passed／1 ignored）。
