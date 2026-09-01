# transport.rs 拆進 transport/

## Summary
`src-tauri/src/transport.rs` 5472 行（本體 2198／同檔測試 3274），data-split 之後的次大檔。本體拆成 `transport/` 底下 8 檔：messages／state_view／context／assemble／turns／arrivals／response／client。

原以為的「組裝／解析／傳輸」三段是假的——`gm_system_prompt` 被行 1192、`gm_dynamic_block` 被行 1216 呼叫，前 528 行其實是下游的被呼叫端。按實測依賴方向重排後無循環：`messages → state_view → context → turns → assemble`，`arrivals → messages`。叫 `turns.rs` 不叫 `lanes.rs`，避開 crate 根既有的 `lanes.rs`。86 支測試同步搬，夾具抽 `transport/test_support.rs`。完整規格見 .ai/plans/transport-split.md。

## Progress
2026-08-26 與 data-split 一起立案。規格經 Sol 兩輪討論定案，依賴鏈無循環已由主線實測三條（state_view 不回呼 context、turns 不呼叫 assemble_*、assemble 呼叫 chars_lane_turn 故位於 turns 上游）。尚未動工。

## Next action
等 data-split 完成並 commit 後才動工；屆時沿用 data-split 的基準抓取與五項驗收流程。

## Constraints
- 純搬家，production body 一個字不改。
- 白名單只有三項：`transport/test_support.rs` 新檔、測試專用可見度、module/import plumbing。
- 不與 data-split 綁同一波實作。
