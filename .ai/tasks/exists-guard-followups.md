# 其餘 exists() 守衛把查詢錯誤當不存在

Status: todo

## Summary
remove-path-try-exists 修了資料層刪除路徑；範圍外同型寫法 6 處仍用 `exists()`，查詢錯誤（無搜尋權限等）會被當成「不存在」：`updater/store.rs:161` `remove_path`、`store.rs:254/265`、`updater/previous.rs:142`、`updater/residue.rs:123`、`updater/launch.rs:46`、`data/world_file.rs` `move_aside`（可能回 `Ok(None)`）。另案處理〔模型判斷·未裁決〕。

## Next action
未排程。逐處判斷是刪除結果判斷或盡力清理，前者改 `try_exists`／直接刪判 NotFound。
