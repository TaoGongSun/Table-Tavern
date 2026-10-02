# desktop-update-detect — 桌面版 App 內更新與回退

分支：`desktop-update-detect`（已 push）

## 現況
- 設計已拍板，規格在 [plans/desktop-update-detect.md](../plans/desktop-update-detect.md)。Mac 只出 Apple Silicon。
- 包 1–4 完成，都經 Opus、Sol 驗收三方共識（2026-10-01／02）：發版管線、格式版本與遷移、偵測與一鍵更新、版本庫與回退。實作細節見計畫各包「實作時補定的細節」。`npm run verify` 全綠（vitest 206、cargo test 687）。
- tauri 與 npm `@tauri-apps/api`／`cli` 都在 2.12.1，要一起升。
- 程式位置：更新與版本庫在 `src-tauri/src/updater/`，command 在 `commands/update.rs`、`commands/versions.rs`，桌備份在 `data/format/backups.rs`；前端 `src/features/updater/` 的 `useUpdateController`、`useVersionStoreController` 已接進 `App.tsx`，不渲染。
- Windows 專屬碼本機編譯不到（ring 需 Windows C 標頭），靠 `test-v*` 演練驗；Mac 真檔解壓測試是 `#[ignore]`，用 `TT_REAL_APP_TARBALL` 指向演練產物的 `.app.tar.gz` 手動跑。
- 待作者裁決：唯讀桌能不能刪（目前可以）。
- 包 5 還沒有程式。包 2–5 首次公開時必須同版發出；計畫「實機驗證」「驗收」要等有兩個真 release。

## 下一步
- 包 5（介面）：先寫做法（首頁更新標記、設定頁版本區與儲存空間區、更新提醒三種強度、回退確認），送 Sol、Grok 審到共識，再派 Grok 實作。畫面位置屬玩家可見的設計，拿不準的先記下問作者。

## 注意
- 測試不碰使用者真實資料目錄與 `/Applications`，一律用臨時目錄；不要走 `install_guarded` 的成功路徑（會把閘門留著），閘門時機用 `run_gated_local`。
