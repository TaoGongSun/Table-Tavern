# refactor_ai.rs 拆進 refactor_ai/

狀態：2026-09-03 階段一（依賴複核）完成並驗收，等階段二施工。

## 現況
`src-tauri/src/refactor_ai.rs` 2764 行（production 1–1810／同檔測試 1811–2764），mechanism-split 結案後本體最大的檔。做法整套沿用 [mechanism-split](../plans/mechanism-split.md)：純搬家、production body 逐 byte 不動、`mod.rs` 只當 facade、零呼叫端的 re-export 不掛。

走 `claude-with-chatgpt` 技能分四階段（依賴複核／搬底層／搬上層加 facade／搬測試），一階段一輪，避開網頁版 25 分鐘砍斷線。

階段一產出＝[規格檔](../plans/refactor-ai-split.md)（380 行），由 ChatGPT 網頁版在 `chatgpt-collab` 完成（`5e09044`），已 cherry-pick 進 main。內容：94 個頂層 item 盤點（34 pub／58 private／2 impl）、內部依賴 DAG 與環檢查、9 檔切線定案、34 個 pub item 的呼叫端清查。

拍板結論：**共用型別集中 `types.rs`**（被 6 個實作模組直接 import，塞 `mod.rs` 會讓 facade 變實作層）。**唯一零呼叫端＝`RefactorAbsorbOutcome`**，拆後留在 `types.rs` 當回傳型別但 `mod.rs` 不掛 re-export。模組 DAG 無環：`types` 為底，`survey`／`expand`／`rewrite` → `prompt_common`，`survey_parse`／`result_parse` → `parse_common`。

驗收抽驗三條全對：本體 `pub` 頂層 34 個；`RefactorAbsorbOutcome` 全庫僅宣告／回傳型別／建構三處，零外部呼叫端；`PrescanSignal` 其他 `.rs` 零命中但同檔測試直接建構（故非零呼叫端），`prescan_worldbook` 有 `commands/refactor.rs` 與 `refactor_assemble.rs` 兩個呼叫檔。

## 下一步
階段二＝搬底層無依賴的檔（`types.rs`、`parse_common.rs`、`prompt_common.rs`），再一輪。交辦時把[規格檔](../plans/refactor-ai-split.md)的切線表與施工紅線指給 ChatGPT，並先抓拆前 baseline 存 scratchpad（頂層 item 清單含可見度、對外簽名、測試函式名 multiset 與總測試數）。

## 界線
- 純搬家：不趁本案收斂重複、拆函式、改命名或改邏輯，不順手整理 prompt 常數或 parser 容錯。
- facade 保住拆前所有**有呼叫端**的 `refactor_ai::X` 路徑與可見度；零呼叫端者不留。
- 可見度只做編譯器逼出來的最小放寬（`pub(super)` 優先）。
- 不改 `commands/refactor.rs`／`refactor_assemble.rs`／`refactor.rs`，呼叫端路徑靠 facade 保持。
- 只推協作分支，合併回 main 要使用者拍板。
