# refactor-apply-open-draft 方案

> 結案 2026-10-11：已進 main，三方驗收通過。

## 現況
- 世界設定裡會重寫世界書的重構操作有兩條，都經 `WorldEditor.refreshAfterApply`：
  - **套用重構卡**（`refactor_apply` → `refactor/apply.rs apply_into`）：對既有條目只會刪掉被完整消耗的來源條目、把整條淘汰的條目停用（只動 `disabled`）；其餘是新增條目。
  - **重新重構的清回**（`refactor_reset_to_source`）：整桌換成照匯入原檔重建的版本，每條都可能換掉。
- 原本開著的既有條目草稿：被停用的那條改過任一欄後，自動存把停用蓋回啟用；清回後自動存用重構後的草稿蓋掉原文；被刪的有後端 `worldbook_entry_gone` 防線。
- 同類：帳本「照原文送模型」開關翻 `disabled` 後，同條的草稿會蓋回；去重併入留下那條的可見度，同條的草稿會蓋回。

## 做法
1. **後端回報套用的條目改動**：`RefactorApplySummary.entry_changes: Option<EntryChanges>`，沿用撤銷的型別與 `receipts::undo_changes::diff`（模組與 `diff` 開成 `pub(crate)`）。`apply_into` 世界書寫完後再讀一次，與開頭快照比；停用報 `enabled`、刪除報 `deleted`、新條目不報；後讀失敗回 `None`、只記 log，不讓已成立的套用回錯。
2. **前端定基**：`refreshAfterApply(live, entryChanges)` 改為 `worldbook.applyWorldChange([], [entryChanges])` 再 `onRefactorApplied`。套用傳 summary 的值；`null`（清回、或套用後讀不到）關掉開著的既有條目草稿。衝突以重構結果為準〔模型判斷·未裁決〕；不走 navigation 的 WorldChange 通知〔模型判斷·未裁決〕。
3. **清回**：確認後、送 `refactor_reset_to_source` 前先存並關掉開著的既有條目草稿（`flushAndCloseExistingDraft`：改過的先存，乾淨的也關；存不起來就不清回），清回失敗時改動也保得住；新條目草稿不動；回來後照 2 傳 `null`。後端重建把原桌 world.md 照抄進新桌〔作者裁決 2026-10-11〕，檔不在當空白、其他讀取錯誤就不清回；前端不必重讀 world.md。
4. **帳本開關**：upsert 成功後、重讀前併一筆 `{uid, ["enabled"]}` 待定基。
5. **去重**：確認後同樣先 `flushAndCloseExistingDraft`，存不起來就不去重；後端不動〔模型判斷·未裁決，依罕見輕防〕。

## 掛點
- 後端：`refactor/types.rs`、`refactor/apply.rs`、`refactor/reset.rs`、`data/world.rs`（`create_reset_world` 收 world.md）、`receipts.rs`＋`receipts/undo_changes.rs`。
- 前端：`features/refactor/refactor-review.ts`、`useRefactorWorkflow.ts`（`flushDraft` 注入）、`features/worldbook/WorldEditor.tsx`、`useWorldbookEditor.ts`。

## 測試
核心回歸測試在舊碼上會紅，另有守門測試（沒命中的既有草稿、新條目草稿照留）。
- Rust：`refactor/tests/entries.rs apply_reports_changed_existing_entries`（停用 `["enabled"]`、刪除進 `deleted`、新增與沒動的不報；沒淘汰沒刪除是空）；`rerun.rs reset_restores_the_freshly_imported_table` 加清回後 world.md 不變；`reset_copies_world_md_only_when_readable`（world.md 讀不了就不清回、原桌不動；檔不在照常清回成空白）。
- `refactor-workflow.test.tsx`：清回傳 `null`、先存草稿再清回、存不起來不清回；套用把 `entry_changes` 交給刷新。
- `WorldEditor.refactor-apply.test.tsx`：被停用條目髒草稿留字且存成停用；乾淨草稿顯示停用、仍乾淨、離開零 upsert；被刪條目草稿關、不問、零 upsert；沒命中的既有草稿照留（守門）；新條目草稿照留（守門）；重讀未完成時切條目等定基才寫；`entry_changes: null` 關草稿；清回先存改過的既有草稿再清；乾淨既有草稿在送出清回時已關、零 upsert；新條目草稿改字後清回仍在、零 upsert；帳本開關後切條目 payload 帶新 `disabled`；去重前先存並關草稿；存草稿失敗就不送去重。
- 既有 mock 補 `entry_changes`：`WorldEditor.test.tsx`、`refactor-workflow.test.tsx`、`card-interface/refactor-undo-flow.test.tsx`（`backend-text.test.tsx` 只有收據標籤字串，沒有 mock，不動）。

## 驗收：測試通道零 AI（2026-10-11 已過）
新桌匯入 `TestCards/NorthHall.png`（成世界書）；scratch 複製 `NorthHall-structure-statusbar-outcome.json`，拿掉來源 uid 20 的產物條目、`dropped` 加一筆 uid 20 整條淘汰。
1. 開 uid 20 草稿改內文 → 世界書更多操作開檔套用全部 → 草稿內文留著、啟用變未勾 → 切條目 → 檔案裡 uid 20 是 `disable: true`、內文是剛改的。
2. 復原上次匯入 → 開 uid 0 草稿改字 → 再套用 → 草稿關掉、沒有確認框 → 返回 → 世界書沒有 uid 0、沒有剛改的字。

## 已知限制（罕見不嚴防）
- 套用排隊等回合期間關掉再重開世界設定：新開的那個收不到套用結果，清單停在套用前，開著的草稿可能把停用蓋回（舊有行為；被刪條目有後端防線）。結果框開著時是模態，排隊與送出到回應之間介面碰不到，不另防。
- 套用失敗時嘗試自動回滾（回滾插回不換 uid），前端不重讀；回滾也失敗（`RefactorApplyPartial`）時世界書停在半套用，開著的草稿可能蓋回停用。
- 清回一律關掉開著的既有條目草稿；改過的已在清回前存檔。
- 帳本開關的 upsert 還在途時切條目：待定基在 upsert 回來後才併，這段空窗裡的自動存可能把開關蓋回（維持現狀〔模型判斷·未裁決〕）。
