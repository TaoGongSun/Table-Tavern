# 匯入卡的世界書角色自己看不到

Status: done〔作者裁決 2026-10-07：立案，列為網頁版公開前門檻〕。已合併 main（2026-10-10）。方案見 [plans/worldbook-character-visibility.md](../../plans/worldbook-character-visibility.md)（P1–P4 作者拍板、第 1–5 輪三方審查已納入）。

## 現況
- 一包施工完成並合併 main；`npm run verify` 12 步全綠、cargo test 1352。
- 落點：可見度預設、去重合併、來源卡與鷹架標記在 `src-tauri/src/data/worldbook/book_import.rs`；原始條目讀寫（重構套用、撤銷插回、匯出）在 `data/worldbook/raw_entries.rs`；重構來源核對與新條目形狀在 `refactor/sources.rs`；匯出 V2 轉換與開場白反解在 `import/export.rs`。
- 測試：`import/card_book_tests.rs`（送出 messages、世界書路對等、去重、收據、匯出往返、MVU、書壞掉）、`data/worldbook/book_import_tests.rs`、`refactor/tests/visibility.rs`、`import/export/tests.rs` 開場白反解、前端 `worldbook-model.test.ts` 與 `refactor-review.test.ts`。
- 驗收第 1 輪 Sol 三條必改（機制條目優先於 meta、匯回的鷹架沿用來源停用值、世界書壞掉匯出失敗）與自動回滾 Err 分支測試已補。

## 下一步
無。Sol、Grok、Opus 審查兩輪驗收通過，已結案。
