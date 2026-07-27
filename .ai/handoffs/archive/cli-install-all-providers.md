# Handoff: cli-install-all-providers

## Current state
四家 CLI（claude／codex／agy／grok）一鍵安裝已實作完成，cargo test 70/70＋npm build 全綠；待使用者實測安裝鈕後結案。

## Completed
- 後端：`install_agy_cli` 泛化為 `install_cli(provider, messages)`，腳本依供應商帶官方安裝 URL／登入指令／驗證探針（src-tauri/src/lib.rs:39–133）；白名單外 provider 回 Err。agy 維持原流程（無登入指令、600s 輪詢），其餘三家先探針、未過才跑阻塞式登入＋120s 輪詢。
- 腳本 PATH 前置補 `~/.grok/bin`、`~/.codex/bin`（grok 官方安裝落點在 `~/.grok/bin`，不補會探針失敗）。
- 前端：四家未偵測皆顯示「一鍵安裝」鈕，同時間僅允許一家安裝中（src/App.tsx:338–346）；`cliNotDetected` 文案刪「App 不代辦」（與新按鈕矛盾）。
- i18n：`agyInstall*` 六 key 泛化為 `cliInstall*`（zh-TW＋en），供應商名／手動安裝網址插值。
- 測試：四家腳本內容各一測＋quote 逃逸與未知 provider 拒絕（src-tauri/src/lib.rs:614–677）。
- README「開通」段落改為「已有訂閱者，四家皆一鍵安裝」；CHANGELOG（本次新建）補一條。

## Verification
- `cargo test`：`test result: ok. 70 passed; 0 failed`（本機實跑）。
- `npm run build`：`✓ built in 457ms`。
- `grep agyInstall src/ -r`＝無殘留；install_cli 已註冊 invoke_handler（src-tauri/src/lib.rs:591）。

## Remaining / Next action
- 使用者已實測 codex 一鍵安裝：安裝＋登入成功，但「驗證成功」訊息未出現——已修（codex 探針 codex exec 在非 git 目錄拒跑，改 codex login status，commit ce64cf2，cargo 70 綠）；下顆打包版生效
- 使用者實測：AI 設定頁對未安裝的 CLI 按「一鍵安裝」，走完終端機安裝＋登入＋自動驗證（至少驗一家非 agy 的）。
- 實測過後結案，接 test-build-cross-platform 重打包（Mac DMG＋CI Windows 測試包；舊測試產物已於 2026-07-24 刪除）。

## Constraints
安裝全程可見終端機；App 不碰帳密／token；只用官方安裝 script。三家官方通道查證紀錄見 tasks/cli-install-all-providers.md。
