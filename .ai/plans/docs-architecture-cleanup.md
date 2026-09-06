# docs-architecture-cleanup

## 目的

整理已過時的工程起手文件，避免新的開發對話誤把 2026-07-18 的 `KICKOFF.md` 當成現行架構與施工規範。

## 範圍

- 將根目錄 `KICKOFF.md` 封存到 `docs/archive/KICKOFF.md`。
- 保留原始歷史內容，只在封存檔最上方加上「歷史文件／不可作為現行開工依據」說明。
- 新增 `docs/ARCHITECTURE.md`，提供目前程式結構、主要模組責任、資料流與驗證入口的短版導覽。
- 更新 `README.md`、`README.zh-TW.md` 的開發者入口，改指向現行架構文件，不再把 `KICKOFF.md` 當工程起手文件。
- `NewPlan.md` 本輪不改寫；它保留作為 2026-07 起始產品方案與決策背景，現行工程細節以實作與後續 `.ai/plans/` 為準。

## 不做

- 不修改 `.rs`、`.ts`、`.tsx`、`.css` 或其他 production code。
- 不重新設計產品架構。
- 不把所有 `.ai` 歷史決策重寫進架構文件。
- 不刪除 `KICKOFF.md` 的歷史內容。

## 驗收

- 根目錄不再有 `KICKOFF.md`。
- `docs/archive/KICKOFF.md` 完整保留舊內容，並明確標示封存狀態。
- 雙語 README 的開發者段落指向 `docs/ARCHITECTURE.md`。
- `docs/ARCHITECTURE.md` 描述的是目前 repo 的實際模組與工作文件入口，而不是 2026-07 的草案。
