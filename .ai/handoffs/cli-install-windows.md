# Handoff: cli-install-windows（Windows 安裝流程改 Rust 引擎）

Updated: 2026-07-25 13:18 +0800（登入改可見終端機；任務背景與四家查證表見 ../tasks/cli-install-windows.md）

## Current state：四家登入全改可見終端機，待重打包＋真人 OAuth 複測
- **登入回歸「可見終端機」約束**（2026-07-25）：朋友複測卡死的根因＝codex/agy/grok 登入是隱藏背景執行，Gemini 在非互動環境退回「貼認證碼」流程、認證碼無處可貼；且隱藏跑＋app 代開瀏覽器違反「安裝過程必須可見、app 不介入 OAuth」拍板（見 tasks/cli-install-all-providers.md）。修法：`InstallSpec.login` 改為一律 `cmd /C start` 開可見視窗（install.rs:104-127，agy 用 `-p ok` 首跑觸發 OAuth 比照 mac），四家 poll_seconds 統一 600（人速）；整組刪除 headless 機制（LoginMode enum、spawn_streaming/StreamingChild、extract_first_url、InstallProgress.url、lib.rs 代開瀏覽器閉包、前端 URL 連結與 cliInstallOpenUrl 字串），net −380 行；login 階段文案改「已開啟終端機視窗…」。
- **Rust spec 驅動引擎**（73b235e→f73e079）：四家共用 detect→install→login→verify 冪等引擎；探針 30s timeout＋kill_on_drop；spawn 一律 env_remove PSModulePath；安裝指令 ErrorActionPreference=Stop fail fast；進度走 cli-install-progress 事件上 app UI；log 落 data_root/install-logs。安裝階段仍為隱藏執行＋app UI 進度（是否也開視窗未拍板）。
- 舊 CI 全綠與打包紀錄（run 30112216400／30114208497）屬 headless 版，已被本輪取代，須重跑。
- 本機驗證（2026-07-25）：cargo test 73 綠（含 codex 沙盒跑不了的 transport TCP 測試）、npm build 綠、headless 殘留 grep 零命中；mac 路徑零邏輯變更。UI 順手改：一鍵安裝按鈕靠右對齊（App.css `.transport-choice .inline > button`）。

## Next action
1. 等使用者下令：跑 ci-windows-verify（smoke 斷言已改為不撈 URL）→ test-build.yml 重打包 → artifact 轉交朋友（Windows 11、Gemini 訂閱→agy）複測：一鍵安裝→跳出終端機視窗→瀏覽器 OAuth（真互動終端機下預期自動回調，備援＝視窗內可貼認證碼）→偵測變綠。
2. 朋友回報結果：綠＝本任務關閉；紅＝讀 app 的 install-logs（UI 有顯示 log 路徑）修復。
3. 安裝階段是否也開可見視窗、Stage C（每週金絲雀排程、診斷打包按鈕、mac 收編引擎）等使用者拍板，不自動開工。

## 已知限制（記錄不擋）
- 探針用寫死官方落點路徑：使用者手動裝在別處會探針失敗（官方安裝器一律落預設位置，影響僅手動安裝者）。
- grok -p 未登入行為官方無載（已以 30s timeout 防禦）；grok 需 SuperGrok／Premium+ 訂閱，失敗訊息有手動安裝引導。
- CI 的 rust-cache 在 job 失敗時不存檔＝紅輪後下一輪仍冷編譯（≈40 分）。

## 派工紀錄（本輪：登入改可見終端機）
- codex gpt-5.6-terra：五檔實作（install.rs／lib.rs／App.tsx／i18n.ts／CI 步驟名）。事故：違反禁令自建 .ai/ 交接檔，主線已還原刪除。
- 主線：根因定位（隱藏執行＝貼碼流程死路＋違反可見性約束）、規格到欄位級、diff 親審、cargo test＋npm build＋殘留 grep 複驗、按鈕靠右 CSS、交接。

## Constraints（承前）
app 不碰帳密 token；只用官方安裝指令；grok 訂閱門檻不特判；不支援 Windows 的家顯示手動安裝引導；CI 觸發恢復「等使用者下令」常規（目標模式已結束）。
