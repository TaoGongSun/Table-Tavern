# Handoff: remove-path-try-exists

## Current state
已結案。刪檔查詢錯誤（例如父目錄沒有搜尋權限）不再當成已刪：
- `world_file::remove_path_raw` 改 `try_exists()?`，查不到回 Err；刪除時 NotFound 算成功。`delete_world_tree` 主目錄改走 `remove_path_raw`，NotFound 照樣清 sidecar。
- `character::delete_character`、`import/images.rs` 刪角色圖／頭像拿掉 `exists()` 守衛，直接 `commit_world_remove`。刪角色在卡目錄無搜尋權限時回錯（不再回成功＋`worldbook_cleanup_failed`）。
- 其餘 `remove_path_raw`／`commit_world_remove` 呼叫端都把錯往上回或本來就忽略結果；撤銷匯入非嚴格模式照舊吞錯。
- 三條權限測試在 root 下（0o000 擋不住查詢）eprintln 後略過。
- Windows：`try_exists` 遇拒絕存取／共用衝突回「存在」，接著刪除失敗回 Err（delete-pending 檔會回錯）；ENOTDIR、檔名過長回錯〔模型判斷·未裁決〕。

## Next
範圍外同型 `exists()` 寫法 6 處（`updater/store.rs:161` `remove_path`、`store.rs:254/265`、`previous.rs:142`、`residue.rs:123`、`launch.rs:46`、`world_file.rs` `move_aside`）由主線另立待辦 `exists-guard-followups`〔模型判斷·未裁決〕。
