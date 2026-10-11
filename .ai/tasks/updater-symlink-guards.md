# updater 遇到 symlink 的兩個誤判

Status: todo

## Summary
exists-guard-followups 審查發現（既有問題）：
- `updater/residue.rs` `snapshot` 的 `update.exists()` 會跟著 symlink 走；該位置常是 symlink，目標查不到時判成 `UpdateAbsent` 並刪掉紀錄，而上層目錄其實可查可寫。
- `updater/store.rs` `remove_path` 用 `try_exists` 也跟 symlink 走；`.partial-x` 是懸空 symlink 時判成不在而略過，之後 `create_dir` 一直回 EEXIST，該版本永遠下載不了。
機率都低。

## Next action
未排程。改用 `symlink_metadata` 判斷路徑本身是否存在，補測試。
