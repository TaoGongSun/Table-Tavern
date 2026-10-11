# undo-import-table-lock 方案

> 結案 2026-10-11：已進 main，三方驗收通過。

## 現況（修前）
- 匯入（`useImportController` 各入口）一律 `runTableOp(...)`，鎖內才 `turnBackend` 等回合、送後端、`refreshReceipts`。
- 撤銷（`App.undoLastImport`）只過 `undoWait.run`，不取 `runTableOp`；陣容欄「＋」選單的撤銷項與匯入項都不看 `tableOpBusy`。
- AI 回合一開始就 `noteChatStarted` 隱藏撤銷項，所以真正碰得到的是：匯入後端呼叫在途時按撤銷，或匯入／撤銷排在收回、還原等非 AI 忙碌操作後面，兩者等完送後端的順序不固定——可能撤掉剛匯入的卡、確認框卻是上一筆名稱。
- 重構套用、重新重構清回、貼開場白都不取 `runTableOp`，只過各自的 `useTurnWait`。

## 做法
1. **撤銷走 `runTableOp`**：`undoLastImport = () => runTableOp(() => undoWait.run(runUndoLastImport))`，從確認框前持鎖到完成訊息，與匯入同一把鎖，兩方向都擋。`undoWait` 保留給等回合提示與卸載判斷。
2. **選單停用**：`tableOpBusy` 時「匯入卡片」「撤銷上次匯入」兩項停用；撤銷項另在 `openingPost.busy` 時停用（`undoLastImport` 開頭同條件放棄）。不加說明文字。
3. **後端比對防撤錯**：`undo_last_import` 指令必帶 `expected`（確認框那筆收據的 kind／label／timestamp／character_id＋清單長度 count；時間戳只到分鐘，單靠它分不出同分鐘兩筆）。後端最後一筆不符就回 `import_receipt_changed`、什麼都不動；收據已空也回 `import_receipt_changed`。前端顯示「匯入紀錄剛更新過，請再按一次『復原上次匯入』」（十語系）並重讀收據。count 擋新增一筆（含同分鐘同名），不擋等長替換；這擋住重構套用等不拿前端鎖的入口另寫一筆收據〔模型判斷·未裁決〕。玩家撤銷走新的 `undo_expected_import`；不比對的 `undo_last_import` 只剩測試用（`cfg(test)`），重構回滾 `rollback_last_import` 照舊不比對。

## 測試
- Rust `receipts/tests.rs`：確認後別筆又寫收據 → 不撤、兩卡與收據原樣，拿最新那筆再撤照撤；同名同分鐘、count 不同 → 不撤；收據已空 → 帶 expected 回 `import_receipt_changed`、不帶照舊 `no_import_to_undo`。
- 前端 `src/features/characters/undo-import-table-lock.test.tsx`（App 層）：
  - 回歸（舊碼上紅）：匯入在途時撤銷項／匯入項停用、以程式直接觸發撤銷 handler 不呼叫 `confirm`／`undo_last_import`，匯入完成後確認框是新收據且 `expected` 帶 count；撤銷確認框未答時直接派送 file input change 不呼叫 `import_character`；收回進行中匯入在等回合 → 撤銷被擋；撤銷確認後在等回合 → 匯入被擋；後端回 `import_receipt_changed` → 報錯並重讀收據、下次確認框是新的最後一筆；貼開場白在途 → 撤銷項停用、handler 放棄，貼完恢復。函式層用保留真 CastRail 的 mock 記下最新 props 取 `onUndoImport` 直接呼叫；等回合用 `vi.waitFor` 條件等待。
  - `refactor-undo-flow.test.tsx`「撤銷匯入後刷新到一半換桌」三條改驗：撤銷懸著時回不了大廳，放開後才能離桌（換桌晚到改由鎖排除；套用重構那三條照舊）。
  - 防漏放守門（舊碼上也綠）：確認框取消、回合結束後各自完成、`undo_last_import` 回錯顯示錯誤，之後都能再匯入或撤銷。

## 已知限制（罕見不嚴防〔模型判斷·未裁決〕）
- 匯入取鎖前有空窗：讀檔、`probe_import`、身分框／路由框、`tableHasWorldbookEntries`。框是 modal 擋陣容欄，但作答後若鎖被撤銷占著，該次匯入靜默放棄。
- 卸載時要等進行中的 Promise 結束才放鎖。
- 重構套用／清回、貼開場白不取前端鎖；重構套用另寫一筆收據由第 3 項後端比對擋住，開場白由撤銷項停用條件擋住撤銷那一側。
- 同一分鐘內重新重構清回又重建、且五欄（kind、label、timestamp、character_id、count）全同時可通過比對：撤掉的是重建後的同一筆匯入，語意一致。
