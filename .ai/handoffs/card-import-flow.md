# Handoff: card-import-flow

## Current state
包 1 完成並自驗綠。接著派 opus 子代理做包 2（匯入收據＋復原）。

## Completed
- 包 1：世界書匯入（GM 編輯區＋改道路徑）後一律 setSpeaker(GM)；匯入成功後自動名桌改成卡名（probe 新增 name 欄位；純世界書 JSON 退用檔名）；去重不再觸發選 GM。

## Verification
- 包 1：cargo test 317 綠、vitest 17 綠、npm run build ✓、check:i18n 全 OK。改點：[import.rs:29](../../src-tauri/src/import.rs)（probe.name）、App.tsx adoptImportName／hasAutoName（renameTable 附近）、onImported 帶 name、redirect 路徑無條件指 GM。

## Remaining
- 包 2：匯入收據＋復原（派 opus）
- 包 3：單一入口＋類型選擇框（派 opus）
- 包 4：第二張卡路由框（派 opus）

## Next action
派包 2：後端收據落 `worlds/<id>/import-receipts.json`，涵蓋 import_character／import_worldbook 寫入面（含 import_mechanism、import_table_tavern_extension、save_world_card、原始檔、桌名改動），undo 指令逐筆倒退；前端側欄匯入鈕旁「復原上次匯入」。拍板細節見任務檔第 3 條。

## Constraints
- 包 2–4 都動 App.tsx 匯入區與 import.rs，依序執行禁並行。
- 新 UI 字串十語系逐鍵補（zh-TW 正典）；驗證四件套：cargo test＋npm run build＋npm test＋check:i18n。
- 後端已有 rename_world（data.rs:805）、世界書條目有 uid、機制記帳先例 worlds/<id>/mechanism-log.jsonl。
