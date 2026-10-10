# ST 巨集契約

- `st-macros.json`：酒館巨集名單（只是名稱表）。重構骨架的佔位符判定前後端共用（`src/features/refactor/refactor-shell.ts`、`src-tauri/src/refactor_ai/result_parse.rs`）；兩邊引擎的測試也驗證名單上每個名字都有內建巨集（`charjailbreak` 除外）。
- `st-macro-cases.json`：從 SillyTavern 06bde939 `tests/frontend/MacroEngine.e2e.js` 抽出的輸入→輸出案例（user＝User、char＝Character、不代卡欄位）。
- `web-macro-cases.json`：網頁版 `substituteParams` 跑出的預期值，補卡欄位、歷史、時間、`{{pick}}`、變數型別與鍵順序。產生：在 `web/` 底下 `node scripts/gen-st-macro-fixtures.mjs`（輸入 `web/scripts/st-macro-fixture-cases.ts`，預設卡與對話在 `macro-parity-runner.ts`）。時鐘固定、時區固定 Asia/Taipei（+480 分）；`random` 依序取用、用完是 0。`expected` 是輸出、代換後 local／global 變數表的 `JSON.stringify` 原文（比鍵順序與數字寫法）、亂數用量。

兩邊測試只比對、不改寫：網頁版 `web/src/features/sillytavern/macro-engine.test.ts`、`macro-parity.test.ts`，桌面版 `src-tauri/src/st_macros/parity_tests.rs`。改案例或改任一邊實作導致預期值變動＝重跑產生腳本、人工核對差異，兩邊同一筆 commit。網頁版退回 JS `Date` 解析的時間字串（非 ISO 8601／RFC 2822）兩邊結果不同，不收進案例。
