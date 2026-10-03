# quota-insufficient-alert — AI 請求失敗：攔截式彈窗＋送出失敗收回玩家句

Status: awaiting-verification。做法、已知限制與測試清單見 [plans/quota-insufficient-alert.md](../plans/quota-insufficient-alert.md)。

## 現況
已進 main（2026-10-03，Sol 審查與驗收同意；verify 全綠 vitest 699、cargo 820、harness 28）。
- 回合（送出、請角色發言、GM 旁白、GM 推進）失敗一律開 `TurnFailedDialog`，只有「關閉」；生圖對話框維持原樣。
- 打字送出只在乾淨路徑（玩家句帶收據落檔、之後回覆失敗、沒換桌）由後端 `discard_unanswered_player` 條件式截檔並把原文放回輸入框；其他情況保留玩家句，原文以唯讀文字放在彈窗。
- 逐字稿寫入加同檔鎖、追加失敗截回、讀者略過殘段（`world_file.rs`、`scene/transcript.rs`）。

## 下一步
實機（排在[實測佇列](../reference/verification-queue.md)梯 1）：彈窗外觀與焦點、卡片覆蓋層開著時彈窗疊在上面、唯讀原文可選取複製、打字送出失敗後輸入框還原。過了就結案歸檔。
