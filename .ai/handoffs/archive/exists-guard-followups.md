# Handoff: exists-guard-followups

## Current state
已結案。同 remove-path-try-exists 修法：查不到在不在回 Err，刪除遇 NotFound 算成功。

逐處判斷（全部屬「判斷刪除／搬移結果」，沒有維持原樣的）：
- `updater/store.rs` `remove_path`：`try_exists()?`，刪除 NotFound 算成功。順帶 `clear_all_residue` 的版本庫查詢同改，且改逐筆清：單筆失敗記 log 後清下一筆、最後回第一個錯（Windows 待刪項目拒絕存取不中止整輪）。
- `updater/catalog.rs` `version_dirs`：版本庫查詢改 `try_exists()?`，查錯不再回空清單。
- `store.rs` `commit_download`：舊版目錄 `final_dir` 判斷改 `try_exists()?`（錯了走既有失敗路徑、清 partial）；改名失敗後的 `trash` 搬回是已失敗後的盡力回復，維持 `exists()`〔模型判斷·未裁決〕；成功後清 trash 拿掉守衛直接 `remove_path`。
- `updater/previous.rs` `write_state` 清空：直接 `remove_file`，NotFound 算成功。`read_previous` 讀檔非 NotFound 錯誤改回 `unreadable`（原先 `is_file()` 查不到當沒紀錄）；五處呼叫端行為與當成沒紀錄等價，只多一條 warn（`write_pending` 仍覆寫、`prune_versions` 仍修剪，屬既有策略）。
- `updater/residue.rs` `delete_record`：直接 `remove_file`，NotFound 算成功。
- `updater/launch.rs` `clear_launch_dir`：直接 `remove_dir_all`，NotFound 算成功；刪掉後 versions 必在，直接 fsync。
- `data/world_file.rs` `move_aside`：原檔與備份名探測都改 `try_exists()?`（探測查錯若當不存在會挑到既有備份名蓋掉）。

呼叫端影響（都不會卡死可恢復狀態，原本也只在上層無搜尋權限才觸發）：
- 啟動殘留清理（`lib.rs` `clear_all_residue`）與啟動後整理（`settle_launch` 的 `confirm_on_launch`／`clear_launch_dir`／`cleanup_residue`）：錯誤只 log、各步獨立，下次啟動重試。
- 下載／回退點（`commit_download`、`prepare_download_reuse`）：回錯中止這次下載，原本查錯後 `create_dir(partial)` 也會失敗。
- Mac 安裝前整理：`cleanup_residue` 走到 `UpdateAbsent` 刪紀錄失敗改回錯，安裝不開始、紀錄留著下次再整理；安裝失敗路徑的 `delete_record` 本來就 `let _`。
- `delete_version` 的 `clear_previous_if`：查錯時 read 已回 unreadable、直接不動。
- 世界書重設（`world_info_store` 兩處 `move_aside`）：原本查錯當沒檔、靜默跳過，現在回錯。

未改〔模型判斷·未裁決〕：`residue.rs` `snapshot`／`maybe_delete_previous` 仍用 `exists()`。查錯判 false＝不刪 previous、偏保守；上層共同目錄權限失效時，snapshot 判成 `UpdateAbsent` 會由 `delete_record` 回錯擋下；個別 update 路徑查錯不保證，已列入 `updater-symlink-guards`。

範圍外兩個 symlink 問題，主線另立待辦 `updater-symlink-guards`〔模型判斷·未裁決〕：
- `residue` snapshot 的 `update.exists()` 跟 symlink，目標查不到會判 `UpdateAbsent` 刪紀錄。
- `store::remove_path` 對懸空 symlink 判不在，之後 `create_dir(partial)` 一直 EEXIST。

## 驗證
- 權限測試共用 `src-tauri/src/test_perm.rs` 的 `with_unsearchable`（drop guard 還原權限，panic 也還原；0o000 擋不住查詢時 eprintln 後略過），world_file 兩條同改用它。
- 另測 `clear_all_residue` 單筆卡住其餘照清、previous.json 是目錄時 unreadable 且啟動確認不寫檔。

