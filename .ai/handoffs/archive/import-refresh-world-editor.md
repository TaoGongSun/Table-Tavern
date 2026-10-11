> 結案 2026-10-11：已進 main，三方驗收通過。

# import-refresh-world-editor

世界設定開著時從陣容欄匯入卡，條目清單不重讀。規格見 [plans/import-refresh-world-editor.md](../../plans/import-refresh-world-editor.md)。

## 狀態
- 實作完成：後端回報 `widened_entries`，匯入完成以 WorldChange 通知世界設定重讀，擴大可見度的既有條目走待定基；通知欄位改名 `entryChanges`／`EntryChanges`。
- `npm run verify` 12 步綠（cargo test 1639、vitest 1241、web vitest 779）。
- 測試通道零 AI 實測三步全過（截圖在施工代理 scratch `irwe/shots/`）：匯入 Y 後清單出 Y 條目、勾選名單有 Y、world.md 與新條目草稿留著；被擴大的共用條目草稿可見度跟上、改過的內文留著、切條目自動存後檔案相符；匯入純世界書後新條目出現。
