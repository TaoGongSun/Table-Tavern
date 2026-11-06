# room-table-chat — 桌邊聊天

Status: 等合併（Sol 方案審 2 輪、驗收 2 輪 PASS、截圖主線親看；Grok 寫碼；與 room-status-strip、room-round-timer 並行〔作者 2026-11-06〕）
Created: 2026-11-06
案別：中案偏大，核心估約 290 行；信任邊界輸入，方案先審。規格見 [plans/room-table-chat.md](../plans/room-table-chat.md)。

## Summary
玩家回饋第 3 案：「聊天」分頁，對象選大家或某位成員（私訊）；不進故事、GM 提示詞、匯出；伺服器留最後 200 則，快照與重連 live 依收件人過濾，即時 vol 只送成員連線；分頁未讀數字、故事頁一行提示。倒數中不關聊天。

## 下一步
1. 方案審過後 Grok 施工（分支 `room-table-chat`，worktree），主線讀 diff、跑 verify、親看截圖。
2. 結案時注意：協定有新指令與欄位，先 `wrangler deploy` 再打包（等作者）。
