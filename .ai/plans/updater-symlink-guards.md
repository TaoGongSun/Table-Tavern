# updater-symlink-guards 方案

Status: 已結案（2026-10-11 合併 main）

範圍只修 `updater/store.rs`；`residue.rs` 不改〔模型判斷·未裁決，依作者「罕見邊際狀況輕防」〕。

## 現況（修前）
- `remove_path` 用 `try_exists`（跟 symlink）：`.partial-<版本>` 是懸空 symlink 時判不在、略過，`commit_download` 的 `create_dir(partial)` 永遠 EEXIST，該版本永遠下載不了。
- `commit_download` 判 `final_dir` 也用 `try_exists`：懸空時 `rename(partial, final_dir)` 一樣永遠失敗。
- `remove_path` 的呼叫者只有 `clear_version_residue`、`clear_all_residue`、`commit_download`，對象都是自家 `.partial-*`／`.trash-*`。

## 改法
- `entry_exists(path)`：`symlink_metadata` 判路徑本身，NotFound→false，其他錯回傳；`commit_download` 判 `final_dir` 用它，懸空連結先改名成 trash 再刪。
- `remove_path`：一次 `symlink_metadata`（NotFound 算已刪、其他錯回傳），依取得的類型分流：真目錄 `remove_dir_all`；symlink 先 `remove_file`、失敗再 `remove_dir`（Windows 目錄 symlink／junction），兩者都失敗回 `remove_file` 的錯；其他 `remove_file`。刪除時 NotFound 算已刪。

## 測試（`#[cfg(unix)]`，連結消失一律以 `symlink_metadata` 得 NotFound 斷言）
1. 懸空 `.partial-1.0.0` → `commit_download` 成功、版本目錄是真目錄、`.partial` 消失（舊碼紅）。
2. 懸空正式版本目錄 → `commit_download` 成功、換成真目錄、`.partial`／`.trash` 消失（舊碼紅）。
3. `.trash` 指向含檔真目錄、`.partial` 指向真檔案 → `clear_all_residue` 只刪連結，目標都還在。

## 已知限制
- residue 不修：`.TableTavern-update.app` 懸空 → `exists()` 判不在、刪紀錄並放行，安裝解壓失敗一次；`macos.rs` 失敗善後 `remove_dir_all` 刪連結成功時自癒。`Table Tavern (previous).app` 懸空 → 判不在走 `RenameToPrevious`，改名回 ENOTDIR：安裝前整理回錯擋安裝，啟動後整理只記 log，需手動刪連結。兩路徑正常流程不會產生懸空連結。
- 版本庫目錄本身是懸空 symlink 時 `create_dir_all` 永遠 EEXIST，不修。
- `commit_download` 改名失敗時，懸空的舊 final 留在 `.trash-*` 不還原，下次清殘留處理。
- Windows 不跑 symlink 測試（建 symlink 需權限），symlink 刪除分流在 Windows 未實測。
