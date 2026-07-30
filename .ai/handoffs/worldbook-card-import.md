# Handoff: worldbook-card-import

## Current state
匯入實作完成、自驗全綠；追加條目「就地展開編輯」UX 改版也完成並瀏覽器實測通過。等使用者實機驗收後結案。

## Completed
- 後端：`import::worldbook_json`（src-tauri/src/import.rs:407 起）——PNG 先解 chara chunk，角色卡剝到 `character_book` 層；`import_worldbook` 命令改收位元組（src-tauri/src/lib.rs:288）。
- 前端：匯入改傳位元組、選檔放寬為 .json/.png（src/App.tsx）；刪除已無人用的 `worldbookReadError` 字串（十個語系檔）。
- 測試：`worldbook_json_unwraps_lorebook_cards`（import.rs 測試模組末）蓋 PNG 卡／JSON 卡／一般世界書 JSON 三路，含 keys=null 常駐條目。
- UX 改版（2026-07-30 使用者回饋：表單固定頂端，點下方條目的編輯看不出反應）：條目編輯改就地展開——編輯取代該列、新增在清單底部展開並自動捲到可見，表單抽成 `entryForm` 共用，儲存／取消鈕移到表單頂端（src/App.tsx WorldEditor）。

## Verification
- `cargo test`：117 passed, 0 failed。
- 真卡煙霧（TestCards/b3d7fd3600ab58d3252e8b38340390c4.png，臨時測試已移除）：`real card imported 17 entries`，抽查條目標題「世界观」「app-求治者」等與 constant 旗標正確。
- `npm run build` exit 0（UX 改版後重跑仍綠）。
- 就地展開瀏覽器實測（vite＋暫時 Tauri stub，17 條假資料，已清）：點條目 10 編輯→表單原地取代該列；取消→17 列復原；新增→表單在清單底部展開且完整入視野（getBoundingClientRect fullyVisible: true）。
- 踩雷已修：新掛的 useEffect 一度放在元件早退 return 之後，觸發 Rules of Hooks 全黑畫面——已移到早退前；scrollIntoView 的 smooth 會被後續 render 打斷停在半路，改瞬間捲動。

## Remaining / Next action
- 使用者實機：世界設定 → 世界書「匯入」選該 PNG → 確認 17 條入列；點下方條目「編輯」確認就地展開；「新增條目」確認底部展開並捲到可見。回報後結案。

## Constraints
- 匯入併進當前開啟的桌，不自動開新桌（2026-07-30 與使用者確認現狀即此，如要「一鍵成新桌」另開任務）。
