# receipts-split — receipts.rs 拆檔（已結案）

`src-tauri/src/receipts.rs` 依功能拆成：父檔留型別、快照、收據讀寫、指紋；`receipts/record.rs` 記帳；`receipts/undo.rs` 復原。純搬動，公開路徑以 `pub use` 維持。

未處理的命名疑點：`write_receipts` 只是 `pop_receipts` 的別名；`append_receipt` 與 `append_receipt_atomic` 差別只在讀壞時重寫或放棄，命名看不出來。
