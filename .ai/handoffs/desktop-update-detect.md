# desktop-update-detect — 桌面版 App 內更新與回退

分支：已併入 main 並刪除（2026-10-02）；端對端驗收發現問題再另開分支修。

## 現況
- 設計已拍板，規格在 [plans/desktop-update-detect.md](../plans/desktop-update-detect.md)。Mac 只出 Apple Silicon。
- 包 1–5 完成，都經驗收共識（2026-10-01／02）：發版管線、格式版本與遷移、偵測與一鍵更新、版本庫與回退、介面。實作細節見計畫各包「實作時補定的細節」。`npm run verify` 全綠（vitest 249、cargo test 693）。
- tauri 與 npm `@tauri-apps/api`／`cli` 都在 2.12.1，要一起升。
- 程式位置：更新與版本庫在 `src-tauri/src/updater/`，command 在 `commands/update.rs`、`commands/versions.rs`，桌備份在 `data/format/backups.rs`；前端 `src/features/updater/`：`useVersionCenter` 組合兩支 controller，畫面是 `VersionTab`（設定「版本」分頁）、`VersionStoreSections`、`UpdateReminders`（側欄橫幅、格式轉換啟動對話框）；外部要求切設定分頁走 `features/settings/useRequestedTab`。
- Windows 專屬碼本機編譯不到（ring 需 Windows C 標頭），靠 `test-v*` 演練驗：包 5 演練四項全綠（2026-10-02），tag 已刪。Mac 真檔解壓測試是 `#[ignore]`，用 `TT_REAL_APP_TARBALL` 指向演練產物的 `.app.tar.gz` 手動跑。
- 唯讀桌可刪，刪除確認窗加「有比目前版本新的紀錄，刪掉後更新回新版也找不回來」警告〔作者裁決 2026-10-02〕，已做。
- GUI 煙霧測試過（2026-10-02，release 包＋空白設定）：版本號鈕、版本分頁三區、手動檢查失敗顯示原因、空版本清單、桌備份清單與刪除確認、唯讀桌刪除警告與連同備份刪除都正常；修掉大小 0 顯示成 1 KB。小點、橫幅、啟動對話框要有真新版才看得到，留給端對端驗收。
- 包 2–5 首次公開時必須同版發出；計畫「實機驗證」「驗收」要等有兩個真 release。

## 下一步
- 功能端對端驗收併在計畫「驗收」的兩個真 release 那一輪，第一個帶更新功能的正式版發出前必須驗過〔作者裁決 2026-10-02〕。
- 畫面位置與為過按鈕寬度選的譯詞（法文「更新」Installer、德文 Updaten／Nochmal、西葡「重試」Repetir）移交 ui-redesign 一併重定〔作者裁決 2026-10-02〕；已記進 [ui-redesign](ui-redesign.md) 交接檔。

## 注意
- 測試不碰使用者真實資料目錄與 `/Applications`，一律用臨時目錄；不要走 `install_guarded` 的成功路徑（會把閘門留著），閘門時機用 `run_gated_local`。
