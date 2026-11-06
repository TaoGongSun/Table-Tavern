# room-round-timer — 回合倒數由房主當場開關

Status: 等合併（Sol 驗收 2 輪 PASS、截圖主線親看；與 room-status-strip、room-table-chat 並行〔作者 2026-11-06〕）
Created: 2026-11-06
案別：小案，核心估約 80 行。規格見 [plans/room-round-timer.md](../plans/room-round-timer.md)。

## Summary
玩家回饋第 2 案：房主回合列「⏱ 倒數 ▾」當回合開 1／2／5 分鐘、倒數中可停止；伺服器加 `round.timer` 只改 `deadline`／`pausedRemainingMs` 並發既有 `round.deadline`；玩家行動格上方「房主開了倒數」橫條；房主頁設定改名「每回合預設倒數」。到期行為沿用現況。

## 下一步
1. Grok 照方案施工（分支 `room-round-timer`，worktree），主線讀 diff、跑 verify、親看截圖。
2. 結案時注意：伺服器有新指令，先 `wrangler deploy` 再打包（等作者）。
