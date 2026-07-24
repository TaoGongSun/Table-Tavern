# Handoff: cli-install-windows（Windows 安裝流程改 Rust 引擎）

Updated: 2026-07-25（目標模式完成；任務背景與四家查證表見 ../tasks/cli-install-windows.md）

## Current state：無人化驗證全綠，待真人 OAuth 複測
- **Rust spec 驅動引擎上線**（73b235e→f73e079，已 push）：砍 .ps1 生成；四家共用 detect→install→login→verify 冪等引擎；Headless 登入 spawn_streaming 邊讀邊掃 URL（解 codex login 阻塞死鎖）；探針 30s timeout＋kill_on_drop；spawn 一律 env_remove PSModulePath（pwsh 7 汙染防禦）；安裝指令 ErrorActionPreference=Stop fail fast；進度走 cli-install-progress 事件上 app UI；log 落 data_root/install-logs。
- **CI 驗證（ci-windows-verify.yml，限手動）第 2 輪全綠**（run 30112216400）：windows 原生 cargo test 全過＋四家真裝 smoke 全過（安裝→落點 .exe→未登入探針→OAuth URL 60s 內擷取）。第 1 輪紅因＝PSModulePath 汙染（claude/codex 安裝腳本 Get-FileHash 解析不到），已修。
- **打包完成**（test-build.yml run 30114208497，success）：artifact `table-tavern-windows-unsigned`（msi＋nsis exe，7.4MB），下載頁 https://github.com/TaoGongSun/Table-Tavern/actions/runs/30114208497
- 本機：cargo test 78 綠零警告、npm build 綠；mac 路徑零邏輯變更。

## Next action
1. 使用者下載 artifact 轉交朋友（Windows 11、Gemini 訂閱→agy）真人複測：裝 app→CLI 一鍵安裝→app 內看分階段進度→瀏覽器 OAuth 完成→偵測變綠。
2. 朋友回報結果：綠＝本任務關閉；紅＝讀 app 的 install-logs（UI 有顯示 log 路徑）修復。
3. Stage C（每週金絲雀排程、診斷打包按鈕、mac 收編引擎）等使用者拍板，不自動開工。

## 已知限制（記錄不擋）
- 探針用寫死官方落點路徑：使用者手動裝在別處會探針失敗（官方安裝器一律落預設位置，影響僅手動安裝者）。
- grok -p 未登入行為官方無載（已以 30s timeout 防禦）；grok 需 SuperGrok／Premium+ 訂閱，失敗訊息有手動安裝引導。
- CI 的 rust-cache 在 job 失敗時不存檔＝紅輪後下一輪仍冷編譯（≈40 分）。

## 派工紀錄（本輪目標模式）
- codex gpt-5.6-terra：install.rs streaming 修復實作。
- haiku：codex login status／grok login／grok -p／codex headless 四點查證（前兩者官方文件證實）。
- 主線：規格、審查、死鎖與 PSModulePath 兩個根因定位、防禦小改、CI 輪次操作。
- 事故：內部 sonnet 發包連兩次 spawn 卡死（使用者改令特例派 terra）；一個殘留卡死進程已終止。

## Constraints（承前）
app 不碰帳密 token；只用官方安裝指令；grok 訂閱門檻不特判；不支援 Windows 的家顯示手動安裝引導；CI 觸發恢復「等使用者下令」常規（目標模式已結束）。
