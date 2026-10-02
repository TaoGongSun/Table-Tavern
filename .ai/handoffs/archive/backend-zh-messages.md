# backend-zh-messages — 後端寫死繁中訊息改回傳代碼（已結案）

規格與盤點見 [plans/backend-zh-messages.md](../../plans/backend-zh-messages.md)。

## 結果

- 後端給畫面看的訊息改成 `TTMSG:`＋JSON 代碼（`src-tauri/src/ui_msg.rs`，139 碼），前端 `src/shared/ui/backend-text.ts` 在顯示時依語系翻譯；state 與落檔一律存原文。需修復整頁改回傳原因代碼（`RepairReason`）。
- `npm run check:i18n` 會核對 Rust 代碼、十語系鍵與佔位符；新增後端訊息照 `ui_msg.rs` 開頭註解做。
- 範圍 A＋B＋C 類〔作者裁決 2026-10-02〕；D 類（寫進檔案的段標、前綴、匯出標題）另立案 [backend-zh-data-text](../../tasks/backend-zh-data-text.md)。
- 送審與驗收只經 Sol（Grok 額度用完）〔作者裁決 2026-10-02〕；Sol 整案簽收。verify 全綠；macOS release 實機 ru／zh-TW 修復頁照語系顯示。
- 已知限制：降回舊版時，已落檔的代碼會以 `TTMSG:` 原文顯示；舊模型清單快取的「（官方別名）」要重抓清單才換。
