# transport.rs 拆進 transport/

## Summary
`src-tauri/src/transport.rs` 原為 5472 行（本體 2198／同檔測試 3274），data-split 之後的次大檔。2026-09-01 已拆成 `transport/` 底下 8 個 production 領域檔：messages／state_view／context／assemble／turns／arrivals／response／client，另有 facade `mod.rs` 與測試共用 `test_support.rs`。

開工前複核後將 `gm_dynamic_block` 跟 `TreeRender`／`render_state_tree` 一起放 `state_view.rs`，避免為了跨 sibling 呼叫把 state-render implementation detail 整包升可見度。完整依賴、切線與驗收明細見 `.ai/plans/transport-split.md`。

`transport/mod.rs` 維持 facade，外部既有 `transport::X` 路徑全部不變。實際 transport 測試是 **91 支**（86 同步＋5 `#[tokio::test] async fn`），不是開工前舊盤點的 86 支；91 支 body 全數 byte-identical 搬移，夾具抽到 `transport/test_support.rs`。

## Progress
**完成。** 2026-08-26 與 data-split 一起立案；2026-09-01 完成規格複核、實作與完整驗收。正式程式 commit：`8f26fb71f1eeb99ed6d9ffc83de9fe3a4cd20aec`（`refactor: split transport module`）。

production 頂層 item 拆前／拆後皆 76，遺失 0、多出 0、內容變更 0；只有 7 個 sibling helper 因模組邊界最小放寬為 `pub(super)`：`gm_dynamic_block`、`gm_system_prompt`、`language_rule`、`message`、`push_merged`、`split_person_roster`、`system_event_text`。

驗收全綠：facade 52/52 對外項目完全保留；全庫 527 個 test leaf multiset 不變，實跑 523 passed／0 failed／4 ignored；91 支 transport 測試 body hash 全同；`RUSTFLAGS="-Dprivate_interfaces" cargo check --all-targets` 全綠；前端 build 全綠。暫時 workflow／驗收 script 沒有進 main。

## Next action
本案無後續實作。若未來要重構 transport 內部邏輯，另立新任務；不要回頭擴充本次純搬家 scope。

## Constraints（結案核對）
- production body 一個字未改；未趁機重構、改名、收斂重複或改邏輯。
- 白名單只用了：`transport/test_support.rs`；必要的 production 最小 `pub(super)`；`transport/mod.rs` facade、module／re-export／import plumbing。
- `transport/mod.rs` 保住拆前所有對外 `transport::X` 路徑與可見度。
- 正式程式拆檔獨立 commit，未與其他 refactor 綁在同一波。
