# Handoff: cli-install-windows（Windows 安裝流程改 Rust 引擎）

Updated: 2026-07-31 +0800（系統代理自動下傳＋連線失敗白話提示；任務背景與四家查證表見 ../tasks/cli-install-windows.md）

## Current state：v2 包朋友實測影片已診斷完，閃視窗＋設定未儲存兩根因已修，待 CI verify＋重打包
- **系統代理自動下傳＋連線提示（2026-07-31）**：Grok 回報者續報「空白登入視窗→失敗」，本機沙盒實測定根因＝`grok login` 連 auth.x.ai 被牆丟包時吊死不印字；回報者手動在 cmd 設 `HTTPS_PROXY` 本地端口後登入成功——梯子只開「系統代理」時瀏覽器吃得到、CLI 吃不到。治本：新模組 src-tauri/src/proxy.rs 讀 Windows 註冊表系統代理（ProxyEnable＋ProxyServer，兩種格式解析，新依賴 winreg 0.56），掛到 install.rs run_hidden／run_terminal 與 cli.rs run_cli 三處子程序；使用者自設環境變數或 app 內 envs 一律優先（先掛代理後掛 envs，同名覆蓋），PAC 不處理。提示：App.tsx 加 NETWORK_MARKERS（irm 下載錯誤代號＋登入逾時字串）→ 十語系 `cliInstallHintNetwork`（開系統代理／全域 TUN 再試；沒登入完關窗也會看到）。驗證：cargo test 122 綠（含新 5 個解析測試）、tsc＋npm build 綠；winreg 讀取段是 Windows 專屬碼，待 CI verify。
- **回報者 Grok 安裝失敗（2026-07-31，簡中 Windows）**：錯在 xAI 官方 install.ps1 第 205 行 `Remove-Item`——`.grok\downloads\grok-windows-x86_64.exe` 被別的程序鎖住（防毒掃描／grok 還在跑／前次安裝未死），下載寫入與清理都失敗。非本 app 程式碼問題；app 端 PROVIDER_GUARDS 防重複觸發正常。已加前端白話提示：錯誤 detail 含 `RemoveFileSystemItemIOError`／`being used by another process`／`Failed to install` 任一（PowerShell 錯誤代號不受系統語言翻譯，簡中亂碼也認得到）→ 錯誤區塊上方多一行「安裝檔被其他程式佔住…關閉／稍候／重開機再試」（src/App.tsx:213 比對邏輯、i18n 十語系 `cliInstallHintFileLocked`）。驗證：tsc --noEmit 綠（語系逐鍵型別檢查，十語系缺鍵會報錯）。附帶發現記錄不擋：包裝指令的 `$ErrorActionPreference='Stop'`（install.rs:162）會讓官方腳本的清理 Remove-Item 變致命，蓋掉真正的「下載失敗＋網址」訊息。
- **朋友影片診斷（2026-07-26，43s 錄影逐格看完）**：Gemini 登入其實**成功**（影片 32s「安裝與登入已完成」；朋友手動 `agy -p ok` 也有正常回覆）；掛在畫面的紅框「verification failed」是 Grok 的舊錯誤（log 路徑 install-grok-*）。最後不能聊的真因＝**transport 仍是 "api"**：風險勾選框沒勾＋用右上 ✕ 關窗（✕ 只關不存），聊天照走 OpenRouter → 402（帳戶額度見底）。朋友自救步驟：選 Gemini CLI→勾風險→按儲存。
- **「多次閃視窗」根因與修復**：60s 冷卻只擋登入啟動；真正閃的是沒設 CREATE_NO_WINDOW 的 console 子程序——detect_clis 的 `--version`（每次偵測最多 4 閃）、agy/grok `models`、run_cli 每輪聊天、run_terminal 外層 `cmd /C` 多餘黑窗。修：cli.rs 三處＋新 `hidden_output()` helper、install.rs run_terminal 補 `creation_flags(0x08000000)`（內層 start 開的登入視窗不受影響）。
- **設定頁未儲存防護（使用者拍板：刪 ✕，置頂雙鈕）**：SettingsWindow 標頭改「儲存設定」（`form="ai-settings-form"` 觸發原表單驗證，風險沒勾照樣被擋且看得到錯誤）＋「不儲存返回／返回」（依髒態換字）；Escape、overlay 點擊、AI→外觀切分頁一律走同一確認守門（沿用世界書 unsavedLeaveConfirm）；Settings 逐欄比對算未儲存欄位數，標頭顯示 unsaved-hint。i18n 新增 settingsBack／settingsDiscard 兩語系。
- **終端機自動關閉＝不修**（使用者拍板）：`agy -p ok` 一次性執行完就退出、視窗跟著關是設計必然；登入能完成即可，不堅持視窗常駐。
- 驗證（本機 2026-07-26）：cargo test 77 綠、npm build 綠、tsc 綠。Windows cfg 碼（creation_flags 三處新增）本機不編譯（cross-check 卡 ring C 標頭），寫法照抄既有 run_hidden，待 CI verify 把關。

## （前輪紀錄）防重發引擎 v2，2026-07-25 下午
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
- 2026-07-28 併入（原 cli-connected-badge，該案已結）：grok 的 Windows 探針改 `grok models` ＋ `InstallSpec.probe_expect` 字串比對（stdout 須含 `You are logged in`）＋ `pre_probe: true`（src-tauri/src/install.rs:18、245-265、421）。舊探針 `grok -p "ok"` 本機實測 26.0 秒，逼近 `run_probe` 的 30 秒上限，Windows 更慢很可能一直逾時。測試者要驗：已登入時直接打勾不彈登入視窗、未登入時仍正常走登入。這輪重打包要先併入。
1. **打包已完成（2026-07-26 使用者下令）**：verify 綠 run 30165056516 → 打包綠 run 30165448004（commit 194fb86，artifact `table-tavern-windows-unsigned` 7MB）：https://github.com/TaoGongSun/Table-Tavern/actions/runs/30165448004 。同 commit 另含兩項 ui-overhaul 順手修：故事欄行寬填滿（刪 42rem 上限）、OpenRouter 專屬欄位（API key／base URL）只在 API 直連時顯示。Mac DMG 同步重打（ad-hoc，00:22 版，四項修正齊）。
2. artifact 轉交測試者複測，重點三項：①偵測／聊天不再閃黑窗；②設定頁選 Gemini CLI→勾風險→按置頂「儲存設定」→實聊走 CLI 不再 402；③未儲存時按「不儲存返回」有確認框。
3. 2026-07-31 兩輪修正（檔案被佔用提示、系統代理下傳＋連線提示）尚未進包：CI verify → 重打包 → 請 Grok 回報者**清掉手動設的代理環境變數、只開梯子的「系統代理」**走一輪安裝→登入→聊天；聊天必驗（代理有沒有真的傳進聊天子程序只有這關能證明）。
3. 回報結果：綠＝本任務關閉；紅＝讀 app 的 install-logs（UI 有顯示 log 路徑）修復。
3. 安裝階段是否也開可見視窗、Stage C（每週金絲雀排程、診斷打包按鈕、mac 收編引擎）等使用者拍板，不自動開工。

## 已知限制（記錄不擋）
- 探針用寫死官方落點路徑：使用者手動裝在別處會探針失敗（官方安裝器一律落預設位置，影響僅手動安裝者）。
- grok -p 未登入行為官方無載（已以 30s timeout 防禦）；grok 需 SuperGrok／Premium+ 訂閱，失敗訊息有手動安裝引導。
- CI 的 rust-cache 在 job 失敗時不存檔＝紅輪後下一輪仍冷編譯（≈40 分）。

## 派工紀錄（本輪：空白視窗診斷＋系統代理下傳）
- 主線（Fable 5）全程：假 HOME＋假代理沙盒實測定根因（丟包＝吊死不印字）、proxy.rs 實作與三處掛點、十語系提示、cargo test／tsc／build 驗證、交接。無外包（規格已在對話中定案，委派比直寫貴）。

## Constraints（承前）
app 不碰帳密 token；只用官方安裝指令；grok 訂閱門檻不特判；不支援 Windows 的家顯示手動安裝引導；CI 觸發恢復「等使用者下令」常規（目標模式已結束）。
