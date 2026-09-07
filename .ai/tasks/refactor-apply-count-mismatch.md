# Task
Task-ID: refactor-apply-count-mismatch
Title: 重構套用後的「新增 N 條世界書條目」與磁碟實際條數對不上
Status: todo
Created: 2026-09-06T16:00:00+08:00
Updated: 2026-09-06T16:00:00+08:00

## Summary
2026-09-06 跑 world-editor-split 實機回歸時看到的：在結果視窗「展開細看」裡取消勾一個角色與一條世界書條目後套用，完成訊息說「新增 17 個角色・新增 34 條世界書條目」，磁碟上 `worldbook.json` 只有 33 條。角色數 17 有跟著勾選走，條目數沒有。

前端只是把 `refactor_apply` 回傳的 `summary.new_entries` 顯示出來（`src/features/refactor/useRefactorWorkflow.ts` 的 `refactorApplyMessage`），這段在拆分案前後逐字一致，後端該案零改動，所以問題在後端或在我對後端行為的理解。

已知線索兩條，還沒查完：

- `src-tauri/src/refactor/apply.rs:141` 起 `new_entries` 是逐次累加，不是取產物長度。**沒勾的角色不會被丟掉**，會另外寫成一條 `is_person` 世界書條目並且 `new_entries += 1`（apply.rs:166）。所以「34」這個數字本身未必是錯的：32 或 33 條產物條目＋1 條由沒勾的角色轉成的條目都可能湊出 34。
- 產物有 `deletable_shared_uids` 欄位，套用時可能刪掉來源條目；測試桌原本那條自建條目套用後也不見了。

現場沒有留下當時的 `worldbook.json`（接著測第 14 項復原，資料已清空），所以 33 條的組成沒被記下來。

## Next action
- 重現：拿 `TestCards/新的一桌 7-重構卡.json`（18 角色／34 條目／有介面）匯入一張空桌，展開細看取消勾一個角色與一條條目，套用，然後直接讀 `~/Documents/TableTavern/worlds/<id>/worldbook.json`，把 33 條的標題全部列出來——先確認裡面有沒有那個沒勾的角色轉成的 `is_person` 條目，以及少掉的是哪一條。
- 讀完 `apply.rs` 的 entries 迴圈與 `deletable_shared_uids` 處理，確認 `new_entries` 的每一次累加是否都真的對應一次落檔。
- 若確認是計數多算：修正累加點；若確認是條目該寫沒寫：那是資料遺失，優先序要往上拉。
- 開檔面板（匯入選檔）自動化點不到，這一步要使用者手動按。

## Constraints
- 這是後端 `refactor/apply.rs` 的行為，不要順手改前端顯示去遮數字。
- 玩家只在「部分勾選」時看得到這個落差，全部套用時對得上，重現時務必取消勾至少一個角色與一條條目。
