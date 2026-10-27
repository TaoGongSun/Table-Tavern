# undo-two-clicks-summary-line — 收回一下收一樓

## 狀態
已進 main（2026-10-28，Sol、Grok 驗收 DONE）；剩實測佇列梯 2 第 24 項實機驗收。規格見 [plans/undo-two-clicks-summary-line.md](../../plans/undo-two-clicks-summary-line.md)。

- 後端 `pop_transcript_floor`／`restore_transcript_events`（`data/scene/transcript/floor.rs`）一次提交收一樓、整組放回；前端疊為組（`features/play/undo-stack.ts`），後端成功即更新疊，照發起那桌那幕記，重讀失敗或途中換桌不漏記、不重放。
- 舊 `pop_transcript` Tauri 指令已刪，資料層函式只留給測試（`cfg(test)`）。

## 已知限制
- 沒有 `action_id` 的舊登場／私設行無法判定屬於哪一回合，自己成一樓。
- 全 app 只有一疊：回原桌前在別桌追加新內容會清空整疊（`appendEvent` 原有行為），原桌收回的樓就不能復原。
