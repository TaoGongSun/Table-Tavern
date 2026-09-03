# mechanism.rs 拆進 mechanism/

狀態：2026-09-03 結案（commit `cc8c65e`）。

## 結果
`src-tauri/src/mechanism.rs` 2870 行拆成 10 檔，原檔刪除，淨增 96 行（模組宣告與 use）。production 函式本體不動，跨檔存取改 `pub(super)`。

實際 DAG：`types`／`tree` → `rules` → `apply`／`derive`／`triggers` → `ledger`，`parse` 只依賴 `types`。依賴複核推翻了立案時按行號畫的 7 段硬切——`apply_block` 會叫 `recompute_derived`、`derive` 與 `ledger` 互相依賴，故把跨層 helper 下沉 `tree.rs`。

facade 對外 `pub` 17 → 8，砍掉的 9 個逐一查過零外部呼叫端（`parse_updates` 唯一命中是 `transport/response.rs` 的註解）。

驗收：測試函式 48 → 48 名稱逐一相同；`cargo test --lib` 535 passed／0 failed／0 warning。

拆檔由 ChatGPT 網頁版透過 GitHub connector 在 `chatgpt-collab` 分支完成（`a158931`），本機驗證與實測由 Claude 執行。這是 `claude-with-chatgpt` 技能的首案，23.5 分鐘完成。

規格與白名單見 [plans/mechanism-split.md](../../plans/mechanism-split.md)。後續同型任務＝[refactor-ai-split](../refactor-ai-split.md)。
