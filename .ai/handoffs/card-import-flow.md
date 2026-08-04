# Handoff: card-import-flow

## Current state
包 1、2 完成並主線複驗綠。接著派包 3（單一入口＋類型選擇框）。

## Completed
- 包 1：世界書匯入（GM 編輯區＋改道路徑）後一律 setSpeaker(GM)；匯入成功後自動名桌改成卡名（probe 新增 name 欄位；純世界書 JSON 退用檔名）；去重不再觸發選 GM。
- 包 2：匯入收據＋一鍵復原。後端新檔 [receipts.rs](../../src-tauri/src/receipts.rs)（快照 diff 手法；空匯入不留收據；損毀收據回錯不裝沒事；機制／擴充／世界卡殼／桌名全蓋）；三指令 list_import_receipts／undo_last_import／record_import_rename；前端側欄「復原上次匯入」（收據非空才顯示）＋復原後全面刷新＋speaker 落空改 GM；i18n 十語系各補 5 鍵。

## Verification
- 包 1：cargo test 317 綠、vitest 17 綠、npm run build ✓、check:i18n 全 OK。改點：[import.rs:29](../../src-tauri/src/import.rs)（probe.name）、App.tsx adoptImportName／hasAutoName（renameTable 附近）、onImported 帶 name、redirect 路徑無條件指 GM。
- 包 2（主線複驗）：cargo test 326 綠（receipts 9 測試：既存條目保留、玩家改過保留、兩筆連退、損毀不 panic、桌名復原、機制只退自己）、vitest 17、build ✓、check:i18n 78 鈕 OK。抽讀 [receipts.rs:278](../../src-tauri/src/receipts.rs)、App.tsx undoLastImport（4023）／側欄按鈕（5010）／record_import_rename 接點（3591）皆符合規格。

## Remaining
- 包 3：單一入口＋類型選擇框（派 opus）
- 包 4：第二張卡路由框（派 opus）
- 全包完成後：使用者實機驗收（真卡匯入→復原→路由）

## Next action
派包 3：probe 加 has_character／book_entries 兩訊號；importCharacter 三分流（無角色欄位→直接世界書路徑；無內嵌書→直接角色路徑；兩者都有→三選 modal「角色卡／只要世界書／取消」用 lorebook_heavy 預選）；刪舊 importLorebookRedirect 兩鍵框；WorldEditor 移除「匯入世界書」按鈕（保留新增條目／去重／匯出）。

## Constraints
- 包 2–4 都動 App.tsx 匯入區與 import.rs，依序執行禁並行。
- 新 UI 字串十語系逐鍵補（zh-TW 正典）；驗證四件套：cargo test＋npm run build＋npm test＋check:i18n。
- 後端已有 rename_world（data.rs:805）、世界書條目有 uid、機制記帳先例 worlds/<id>/mechanism-log.jsonl。
