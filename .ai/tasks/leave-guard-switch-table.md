# Task
Task-ID: leave-guard-switch-table
Title: 換桌不問未儲存，編輯中的角色卡會靜默丟失
Status: todo

## Summary
角色卡編輯器有未儲存變更時（畫面上已顯示「有 N 項修改未儲存」），從側欄直接點另一桌，App 不會攔，直接換桌，改到一半的內容無聲消失。同一個未儲存狀態下改點側欄的其他角色卡、玩家卡或世界設定，則會正確跳出「尚未儲存 — 確定要離開嗎？」。

2026-08-13 於 app-split 切片 5 的手動回歸中發現。**不是拆分造成的**：`canLeaveEditor()` 的呼叫點在拆分前（591a2e0）與現在都是同樣四處，都在 `src/App.tsx`：
- `editCard`（換編輯對象）
- `openPlayerCard`
- `openWorldEditor`
- `openNewCard`

`switchTable`／`enterTable` 這條路沒接上守門。`canLeaveEditor()` 本身的邏輯是對的（`guarded = cardView !== null || mainView?.kind === "world"`，放行後清掉 `leaveGuard.current`），缺的只是換桌時沒呼叫它。

同一類缺口值得一併確認：刪除桌、`一句話開桌`、AI 生成新桌這幾條也會換掉 `table`，是否同樣繞過守門。

## Next action
在 `switchTable`（以及刪桌／開新桌等會換掉 `table` 的入口）進入點前加上 `await canLeaveEditor()` 的檢查，回傳 false 就中止換桌。

## Constraints
- 這是 app-split 拆分任務期間發現的既有問題，**排在 15 個切片全部跑完之後才動**（使用者 2026-08-13 拍板）。第二批會把桌次生命週期的 handler 重新整理，太早改會撞在一起。
- `canLeaveEditor()` 是 async，換桌路徑上有多個呼叫點，要確認每一條都是 await 之後才動 `setTable`，不要出現「守門還在問、桌已經換掉」。
- 修完手動驗這四條：編輯中換桌、編輯中刪桌、編輯中開新桌、編輯中 AI 生成新桌；各測一次「取消＝留在原地且改動還在」與「確定＝換桌且不再追問」。
