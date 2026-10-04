# Task
Task-ID: ai-table-generator-fixes
Title: 一句話開桌修三處：改大綱白屏、桌名被模型改掉、錯誤顯示原始 JSON

## Summary
2026-10-04 測試通道代測 [ai-table-generator](../handoffs/ai-table-generator.md) 抓到，首發前必做〔作者裁決 2026-10-04〕：
- A：大綱任何文字欄一輸入就白屏。`src/features/lobby/GenerateTableDialog.tsx:247、253、265、278` 在函式型 updater 裡讀 `event.currentTarget.value`，React 19 執行 updater 時已是 null。
- B：開桌後桌名不是玩家草稿的標題。展開提示（`src-tauri/src/genesis.rs:78`）讓模型重寫 `## WORLD:`，`materialize` 直接拿來當桌名；桌名要照草稿標題〔作者裁決 2026-10-04〕。
- C：失敗時原始 JSON 印兩次（`GenerateTableDialog.tsx:327` 的 `<p>` 與 `:328` 的 `<pre>`），露出 OpenRouter user_id；要走 `src/shared/ui/ai-error.ts` 的 `explainAiError` 人話路線。

## Next action
- 修完用 OpenRouter 免費模型重跑交接檔六項驗收（重點③⑤），結案時 ai-table-generator 一併結案。
