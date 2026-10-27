# revert-fork-auto-hidden — revert／fork 復原換幕結算的 auto_hidden

Status: 已結案（2026-10-27 合併 main，Opus、Sol、Grok 三方共識；驗收 DONE）

出處：[ai-card-refactor 交接檔](ai-card-refactor.md) 包 4b1 已知限制。

## 結論
- 做法 A：每幕離開（換幕、分岔）時把角色卡 `auto_hidden` 拍成快照存 `WorldState.scene_card_visibility`，退回／分岔回到該幕照快照寫回；分岔兩段提交（`pending_leave`）保住被離開幕的快照。程式在 `src-tauri/src/data/scene/visibility.rs`，測試 `visibility/tests.rs` 15 支。規格與已知限制見 [plans/revert-fork-auto-hidden.md](../../plans/revert-fork-auto-hidden.md)。
- 驗證：verify 12 步綠；測試通道手工擺出換幕後的桌，GUI 點退回前幕、從前幕繼續，側欄都回到當時。

## 後續建議
- `data/scene/lifecycle.rs` 979 行、測試模組佔六百多行：下次動這檔時把測試搬到 `lifecycle/tests.rs`（比照 `character/tests.rs`）〔模型判斷·未裁決〕。
