# Handoff: cli-install-windows（Windows 安裝流程改 Rust 引擎）

Updated: 2026-07-25 15:0x +0800（防重發認證引擎 v2；任務背景與四家查證表見 ../tasks/cli-install-windows.md）

## Current state：防重發引擎 v2 完成本機驗證，待 CI＋重打包＋Parallels 複測
- **朋友複測二輪紅（2026-07-25）**：認證分頁連環轟炸到近死機＋貼碼終端機神隱。根因三連：①未登入的 agy 每被執行一次就自開 OAuth 分頁（假 HOME 本機實證，等碼上限 60s、回跳走 antigravity.google 貼碼流程非 localhost）；②5 秒探針輪詢＝連環觸發①；③前端 3 秒就誤判完成解鎖按鈕＋後端無併發鎖，重按疊加多條輪詢。
- **引擎 v2（使用者拍板鐵律：每按一次按鈕最多發一次認證，寧可卡住報錯不補發）**：輪詢迴圈整組刪除；流程改 detect→install→（pre_probe 僅 claude/codex，其探針無副作用；agy/grok 禁登入前探測）→`cmd /C start /WAIT "<識別標題>"` 開視窗等結果（上限 600s，kill_on_drop）→視窗失敗＝直接報錯零探針→成功才確認探針（最多 2 次防憑證落盤時差）。守門：try_begin 狀態機（running＋60s 冷卻，RunToken Drop 清旗標）；重按＝AlreadyRunning→只做視窗置頂（windows-sys FindWindowW＋EnumWindows 標題模糊比對＋SetForegroundWindow，開窗後背景任務每 500ms 試 10 次自動置頂）；冷卻中＝Err("login-cooldown:N")→前端 i18n 顯示。mac 補 mac_cooldown（60s 內重按只喚 Terminal 前景）。前端：same-provider 按鈕保持可按（觸發置頂）、done/error 事件收斂 installingCli、cliPollRef 防疊加。
- 驗證（本機 2026-07-25）：cargo test 77 綠零警告（新增 5 流程測試＋2 守門測試）、npm build 綠、`while elapsed` 零命中、run_probe 產線僅 checked_probe 一處。主線修掉 codex 三洞：HWND 在 windows-sys 0.59 是指標非整數（編譯級，mac 測不到 cfg(windows) 碼）、smoke 掉了執行檔存在斷言、前端計時器疊加。
- 已知假設（Parallels 複測項）：claude `-p` 未登入＝本機報錯不發認證（推定）；Windows Terminal 為預設終端時標題比對屬 best-effort。
- **v2 CI 全綠**（run 30149790365：Windows 原生 cargo test＋四家真裝 smoke）；**v2 打包完成**（run 30150174388，artifact `table-tavern-windows-unsigned` 7.4MB）：https://github.com/TaoGongSun/Table-Tavern/actions/runs/30150174388

## （前輪紀錄）四家登入改可見終端機，2026-07-25 上午
- **登入回歸「可見終端機」約束**（2026-07-25）：朋友複測卡死的根因＝codex/agy/grok 登入是隱藏背景執行，Gemini 在非互動環境退回「貼認證碼」流程、認證碼無處可貼；且隱藏跑＋app 代開瀏覽器違反「安裝過程必須可見、app 不介入 OAuth」拍板（見 tasks/cli-install-all-providers.md）。修法：`InstallSpec.login` 改為一律 `cmd /C start` 開可見視窗（install.rs:104-127，agy 用 `-p ok` 首跑觸發 OAuth 比照 mac），四家 poll_seconds 統一 600（人速）；整組刪除 headless 機制（LoginMode enum、spawn_streaming/StreamingChild、extract_first_url、InstallProgress.url、lib.rs 代開瀏覽器閉包、前端 URL 連結與 cliInstallOpenUrl 字串），net −380 行；login 階段文案改「已開啟終端機視窗…」。
- **Rust spec 驅動引擎**（73b235e→f73e079）：四家共用 detect→install→login→verify 冪等引擎；探針 30s timeout＋kill_on_drop；spawn 一律 env_remove PSModulePath；安裝指令 ErrorActionPreference=Stop fail fast；進度走 cli-install-progress 事件上 app UI；log 落 data_root/install-logs。安裝階段仍為隱藏執行＋app UI 進度（是否也開視窗未拍板）。
- 舊 CI 全綠與打包紀錄（run 30112216400／30114208497）屬 headless 版，已被本輪取代，須重跑。
- 本機驗證（2026-07-25）：cargo test 73 綠（含 codex 沙盒跑不了的 transport TCP 測試）、npm build 綠、headless 殘留 grep 零命中；mac 路徑零邏輯變更。UI 順手改：一鍵安裝按鈕靠右對齊（App.css `.transport-choice .inline > button`）。
- **「登入／驗證」常駐按鈕**（2026-07-25 拍板）：偵測只看執行檔在不在，「已裝未登入」時原 UI 無任何登入入口（朋友正卡在此態）。拍板常駐版（否決「未驗證才顯示」——需持久化驗證旗標、有過期死路）：已偵測的 CLI 旁常駐「登入／驗證」鈕，走同一條 `install_cli` 冪等流程，已登入者按下＝幾秒回報 done 當驗證連線用。純前端：App.tsx 已偵測分支加鈕＋i18n `cliLoginVerifyBtn` 兩語系。CI 驗證輪 run 30145802375 全綠（headless 刪除版）；run 30146328876 的 artifact 缺此按鈕已作廢；含按鈕的正式包＝run 30146710122（success，artifact `table-tavern-windows-unsigned` 7.4MB）：https://github.com/TaoGongSun/Table-Tavern/actions/runs/30146710122

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
