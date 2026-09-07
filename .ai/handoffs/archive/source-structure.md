# Handoff: source-structure

規格與逐檔歸位表見 [plans/source-structure.md](../plans/source-structure.md)。

## 結案（2026-09-07）

分支 `repo-hygiene/source-structure`，每段各自 commit、各自跑過 `npm run verify` 全綠。

`src/` 根層從 34 個平鋪檔案清到只剩 `App.tsx`、`App.css`、`main.tsx`、`vite-env.d.ts`。35 檔歸位到 7 個 `features/`（refactor／worldbook／ai-connection／characters／card-interface／import／settings）與 `shared/contracts`、`shared/ui`。`views/world-editor/` 已清空刪除。

`scripts/check-structure.mjs` 是 `npm run verify` 的第 1 步，擋四類違規：根層新增原始碼檔、任意深度的 `utils`／`helpers`／`misc`／`common` 目錄、測試檔沒跟 owner 住、hook 用 `.tsx`。五條規則各以反例試過會紅。**刻意不留豁免清單**：根層已清空就不存在需要豁免的舊檔，一旦開放例外，紅燈的預設解法會變成「把新檔加進清單」，關卡就退化成橡皮圖章。

規範全文在 [docs/STRUCTURE.md](../../docs/STRUCTURE.md)（63 行），`CLAUDE.md` 與 `docs/ARCHITECTURE.md` 都已指向它。

## 全案驗證

`npm run verify` 七步全綠（structure／cargo fmt／vitest 157 測試／i18n／build／cargo check／cargo test 536 測試）。另四項機械檢查：無殘留舊 import 路徑、測試檔仍為 11 支、`.ai/` 現行文件的前端連結全部指得到、`ARCHITECTURE.md` 的 tree 與實際一致。

## 一併發現、未在本案處理

`.ai/` 現行文件有 27 條指向 `src-tauri/src/transport.rs`、`cli.rs`、`data.rs`、`refactor.rs`、`refactor_ai.rs` 的壞連結——那些檔案在更早的 Rust 拆分案裡已變成同名資料夾，連結沒跟著改。非本案造成，已連同掃描指令記進 `.ai/tasks/rust-module-homing.md`。

已 squash 成一筆合併進 main（`58b8cd9`），分支本地與遠端都已刪除，main 上 `npm run verify` 七步全綠。

## 未竟

`views/` 與 `controllers/` 還有約 25 支單一 owner 的檔案沒收，是**已知待收**不是規則例外——checker 判不出 view 的 owner，所以這一區在收完之前不能宣稱被 CI 防住。見 `.ai/tasks/view-layer-homing.md`。
