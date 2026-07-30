# Handoff: worldbook-card-import

## Current state
實作完成、自驗全綠，等使用者實機驗收後結案。

## Completed
- 後端：`import::worldbook_json`（src-tauri/src/import.rs:407 起）——PNG 先解 chara chunk，角色卡剝到 `character_book` 層；`import_worldbook` 命令改收位元組（src-tauri/src/lib.rs:288）。
- 前端：匯入改傳位元組、選檔放寬為 .json/.png（src/App.tsx:1381、1443）；刪除已無人用的 `worldbookReadError` 字串（十個語系檔）。
- 測試：`worldbook_json_unwraps_lorebook_cards`（import.rs 測試模組末）蓋 PNG 卡／JSON 卡／一般世界書 JSON 三路，含 keys=null 常駐條目。

## Verification
- `cargo test`：117 passed, 0 failed。
- 真卡煙霧（TestCards/b3d7fd3600ab58d3252e8b38340390c4.png，臨時測試已移除）：`real card imported 17 entries`，抽查條目標題「世界观」「app-求治者」等與 constant 旗標正確。
- `npm run build` exit 0。

## Remaining / Next action
- 使用者實機：世界設定 → 世界書「匯入」選該 PNG → 確認 17 條入列，回報後結案。

## Constraints
- 匯入併進當前開啟的桌，不自動開新桌（2026-07-30 與使用者確認現狀即此，如要「一鍵成新桌」另開任務）。
