# mechanism.rs 拆進 mechanism/

狀態：2026-09-03 立案，尚未動工。

## 現況
`src-tauri/src/mechanism.rs` 2870 行（本體 1395／同檔測試 1475），transport-split 結案後的次大檔。做法、白名單與機械驗收整套沿用 transport-split（源頭 data-split）：純搬家、production body 逐 byte 不動、`mod.rs` 只當 facade、零呼叫端的 re-export 不掛。

已盤點：本體 56 個頂層 item，對外 `pub` 17 個，其餘 39 個是同檔 private helper。切線草案 7 檔（`parse`／`apply`／`tree`／`rules`／`derive`／`triggers`／`ledger`）與開工前必做三項見 [規格檔](../plans/mechanism-split.md)。

按總行數本檔最大；按本體行數是 `refactor_ai.rs`（本體 1810／總 2764）最大。2026-09-03 使用者拍板先做 mechanism，refactor_ai 留待下一案。

## 下一步
開工首步＝複核依賴方向再定案切線。transport 那案開工前以為的三段式切線是假的（下游函式排在檔案前段），本案的行號分段同樣只是視覺印象，要實際查呼叫關係、畫出 DAG、確認無 sibling 環。同一步順便決定共用型別（`Patch`／`Record`／`Outcome` 等）放 `types.rs` 或留 `mod.rs`，以及 `apply_block` 這個總入口跟不跟 `ledger.rs`。

## 界線
- 純搬家：不趁本案收斂重複、拆函式、改命名或改邏輯。
- facade 保住拆前所有**有呼叫端**的 `mechanism::X` 路徑與可見度；零呼叫端者不留。
- 可見度只做編譯器逼出來的最小放寬（`pub(super)` 優先），不把內部 helper 升成 crate API。
- 正式程式拆檔獨立 commit，不與其他 refactor 綁在同一波。
