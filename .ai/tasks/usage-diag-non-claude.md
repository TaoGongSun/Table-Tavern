# claude 以外的路徑全部只標「單發」，快取診斷等於沒做

## Summary

額度分頁對 API／codex／grok／agy 一律標 `single`（單發），連 81.3% 命中的那幾輪也一樣，等於把「呼叫模式」誤植成「快取診斷」。規格細節見 [.ai/plans/usage-diag-non-claude.md](../plans/usage-diag-non-claude.md)。

## Progress

- 已定位根因與影響筆數，尚未設計標籤體系。

## Next action

開新對話做完整盤點：把 `Diag` 全部標籤逐一對照四條非 claude 路徑，設計新的標籤體系後再動工。

## Constraints

- 只動診斷與顯示，不動組裝與傳輸；與 api-shared-lane 無相依。
- 十語系文案要同步。
