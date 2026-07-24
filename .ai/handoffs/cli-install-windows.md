# Handoff: cli-install-windows（Windows 安裝流程改 Rust 引擎）

Updated: 2026-07-25（目標模式進行中；任務背景與四家查證表見 ../tasks/cli-install-windows.md）

## Current state
- **Stage A 完成並全數 commit**（73b235e → 21d61f4，已 push）：
  - 73b235e：砍 .ps1 生成，install.rs spec 驅動引擎（四家共用，Terminal/Headless 雙登入模式，raw bytes 掃 URL，階段冪等，log 落檔）＋前端事件渲染＋i18n。
  - e06cfd0：**Headless 死鎖修復**——codex login 類指令印 URL 後阻塞等回調，舊版等進程退出才掃 URL＝互等。改 spawn_streaming 邊讀邊掃（500ms/60s 上限），掃到即上屏＋開瀏覽器；輪詢後 kill trigger＋abort reader。補第五劇本 stub＋四家真裝 smoke（#[ignore]）。
  - 2ee6b31：Stage B CI workflow（ci-windows-verify.yml，限 workflow_dispatch）。
  - 21d61f4：探針 30s timeout＋kill_on_drop（grok -p 未登入行為官方無載的防禦）。
- 本機驗證：cargo test 78 綠零警告＋npm build 綠。mac 交叉編譯 windows target 不可行（ring C 依賴），windows 編譯正確性由 CI 原生驗。
- 查證補完（haiku，2026-07-25）：`codex login status` exit0=已登入（官方文件）✅；`grok login` 獨立子指令存在 ✅；`codex login` headless 確認印 URL 後阻塞（issue #2798）＝死鎖修復方向正確；`grok -p` 未登入行為查無法確認（已用探針 timeout 防禦）。

## In flight
- **CI 第一輪進行中**：run 30110562842（windows 原生 cargo test＋四家真裝 smoke，斷在抓 OAuth URL，log 上傳 artifact）。背景監測掛著，紅則修再觸發，修復循環上限 3 輪（使用者已授權目標模式：CI 自主觸發、機械工内部便宜檔、卡關可問 codex sol 討論〔sol 只討論不派工〕）。

## Next action
1. CI 第一輪結果：綠 → 觸發 test-build.yml 打包 Windows 安裝檔，交付 artifact 連結＋驗收報告給使用者（拿給朋友做最終真人 OAuth 驗證）。紅 → 讀 artifact log 修，再觸發（≤3 輪）。
2. 目標模式終點＝安裝檔 artifact 連結＋驗收報告。Stage C（金絲雀排程、mac 收編引擎）等使用者另行拍板。

## 派工紀錄（本輪）
- codex gpt-5.6-terra：install.rs streaming 修復實作（特例：內部 sonnet 發包連兩次卡死，使用者改令派 terra）。
- haiku：四點外部查證。
- 主線：規格、審查、bug 定位、探針防禦小改、commit。

## Constraints（承前）
app 不碰帳密 token；只用官方安裝指令；grok 有 beta 訂閱門檻（識別失敗訊息明講，不特判）；若某家不支援 Windows → 顯示手動安裝引導；CI 打包觸發平時等使用者下令（目標模式期間例外，結束即恢復）。
