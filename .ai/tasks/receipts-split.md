# receipts.rs 拆檔

## Summary

`src-tauri/src/receipts.rs` 已 1227 行。opus-review（stage-card 包 3 驗收）建議：
- `undo_last`（約 870–1225 行）搬到 `receipts/undo.rs`；
- 各 record 函式搬到 `receipts/record.rs`。

純搬動，不改行為。

## Next action

未排程，找一個不動收據的空檔做；拆完 verify 綠即可。
