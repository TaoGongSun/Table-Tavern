# windows-ci-cargo-red：Windows CI cargo test 7 敗

run 37090289248（11eaf0d），windows-latest。CI 是 desktop-update-detect 五包一起推上去才第一次跑到這些測試（056b226 綠 → abb1657 紅），所以它們在 Windows 上從沒綠過，不是產品倒退。

## 判斷：原 7 案全屬 (a) 測試寫法，兩個根因；這 7 個失敗沒有揭露 Windows 的產品缺陷（Sol 審過同意）

### 根因 1：改名注入失敗按「每次嘗試」扣，Windows 的重試把它吃掉（3 案）
- 案：`data::format::tests::rename_failure_leaves_directories_and_retries_next_open`（log：`unexpected open result Ready`）、`refactor::tests::rerun::swap_write_failure_leaves_the_table_untouched`、`swap_failures_after_moving_the_original_away`（log：`reset_to_import_source(...).is_err()` 不成立）。
- 證據：`src-tauri/src/data/world_file.rs:227` Windows 每次改名試 8 次、`:230` 每次嘗試都呼叫 `injected_rename_failure()` 扣一次。`RenameFailGuard::fail(1)` 只讓第 1 次嘗試失敗、第 2 次真的改名成功；`fail_after(3, 2)` 也只打掉同一次改名的兩次嘗試。macOS 只試 1 次，所以「嘗試數＝改名數」，本機綠。
- 產品面：真實的 Windows 改名失敗會重試 8 次後回 `RenameFailedIo`，交換流程照恢復表處理、下次開桌重試，正是這些測試要驗的行為。這 3 個失敗沒有揭露 Windows 的產品缺陷；但其他測試有過，不足以證明 Windows 上所有更新與恢復情境都沒問題。
- 修法：`rename_path` 的注入檢查移到重試迴圈外，skip／fail 都以「呼叫一次 `rename_path`」為單位，命中直接回 `RenameError`。正式路徑的重試、退避與最後的 `RenameFailedIo` 不變。
- 補重試測試（注入移出迴圈後，上面三案不再覆蓋重試）：在每次嘗試都經過的控制點加測試用計數（嘗試數、等待數）與「下 N 次嘗試回 IO 錯」，用計數判斷、不靠時間：第一次失敗第二次成功＝嘗試 2 次；每次都失敗＝Windows 嘗試 8 次、等待 7 次、回 `RenameFailedIo`、檔案沒動；macOS／其他平台嘗試 1 次，預期值依平台寫。

### 根因 2：Mac 替換流程靠目錄識別，非 unix 刻意取不到（4 案）
- 案：`updater::residue::tests::each_interrupted_stage_continues_and_unknown_states_stop`、`leftover_temp_record_is_ignored_and_interrupted_rewrite_keeps_the_old_file`（log：`app_id_unavailable`）、`same_from_version_interrupted_after_swap_drops_the_older_copy`（`app_id(&update).unwrap()` 得 None）、`updater::macos::tests::successful_swap_renames_the_old_version_to_previous`（previous 沒產生：步驟 5 的整理回 `app_id_unavailable`，被 `macos.rs:136` 記 warn 吞掉）。
- 證據：`residue.rs:367` 非 unix 的 `file_id_of` 回 None，`residue.rs:175` 進 swapped 時取不到識別就回 `AppIdUnavailable`（規格：取不到就停、不對調）。正式碼只有 macOS 走這條：`replace_installed_app` 只在 `commands/update.rs:350`、`commands/versions.rs:110` 的 `cfg(target_os = "macos")` 區塊呼叫；啟動後整理吃 `mac_bundle()`（`commands/update.rs:395`），非 Mac 回 None。Windows 更新走 NSIS，不碰這段。
- 修法：依賴 dev／inode、走假 swap 的測試加 `#[cfg(unix)]`，與 `file_id_of` 的 cfg 對齊（只有真的呼叫 `renamex_np` 的測試才限定 macOS）。`leftover_temp_record…` 拆兩案：依賴 AppId 的整理那段限定 unix，正式紀錄原子改寫那段照舊跨平台。`missing_app_id…` 這類不依賴真識別的測試照舊跨平台跑。`residue.rs:39` 的註解寫「Windows 用磁碟序號與 file index」與實作不符，改成「只有 unix 取得到；其他平台照規格停下，相關測試只在 unix 跑」。
- 不採：替 Windows 實作 file id（windows-sys 加 `Win32_Storage_FileSystem`、以 `FILE_FLAG_BACKUP_SEMANTICS` 開目錄取 volume serial＋file index）——只為讓永不在 Windows 執行的 Mac 流程在 Windows 測得到而加正式碼〔模型判斷·未裁決〕。

## 修完 lib 後才露出的 harness 2 案
原本 cargo lib 先敗、verify 中斷，`--features test-harness` 的測試在 Windows 從沒跑到。
- `harness::root::tests::dangerous_roots_are_rejected`：**test-harness 在 Windows 上的路徑穿越缺陷，已修**（不算 (a)）。`canonical_lenient` 接回不存在的尾段時，`PathBuf::push` 會重新解析尾段：verbatim 輸入 `\\?\C:\scratch\alias/x/../sub` 的尾段 `alias/x/../sub` 在這裡才被拆開、`..` 被吃掉，得到 `alias\sub` 卻沒再解析別名；`alias` 若是指向正式資料的 junction，保護檢查認不出，`--fresh` 清理（在 `prepare` 的 symlink 檢查之前）就可能刪到正式資料。修法：尾段每一段重新解析後必須剛好是同名的單一 Normal 段，否則拒絕。選這個而不是「verbatim 輸入含 `/` 就拒絕」：根因在 push 重新解析，這樣 `/`、`..`、`.` 各種重新解析都擋住，`prepare` 經 `check_destination` 的呼叫也一併受保護。測試：原 `{home}/x/../y` 案恢復原樣（Windows 上現在真的驗到）；新增 Windows 限定回歸測試，以 `mklink /J` 在 scratch 下建 junction 指向假正式資料夾，驗 `validate_root` 拒絕且假資料不動（啟動時驗 root 是第一步，先於取鎖、`--fresh` 與任何寫入）。
- `harness::root::tests::lock_is_exclusive_and_file_survives_release`：(a)。Windows 檔案鎖是強制鎖，被鎖時另一個 handle 讀不到持有者，錯誤訊息少了 pid；排他性本身成立。修法：「錯誤帶持有者」的斷言限 unix（能讀取時附上持有者），排他與鎖檔保留照舊跨平台。

## 結論（2026-10-03 結案，Sol 驗收同意）
原 7 案是測試在 Windows 上不成立，已改寫；另修 test-harness 在 Windows 上的路徑穿越。Windows CI run 37094272901 全綠；反證 run 37094277152（拿掉尾段檢查時 junction 回歸測試轉紅）。
