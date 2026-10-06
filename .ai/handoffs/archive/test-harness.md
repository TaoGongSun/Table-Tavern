> 結案 2026-10-02：包 1–3 進 main，Sol 結案驗收通過；剩三項待驗排在[實測佇列](../../reference/verification-queue.md)。

# 測試通道（test-harness）

Status: done

## Summary
測試建置專用的控制通道，讓主線不靠使用者點擊對真 app 跑 GUI 實測〔作者裁決 2026-10-02〕。規格、spike 結果與驗法見 [plans/test-harness.md](../../plans/test-harness.md)。正式包不含任何 harness 程式；前端零改動。

- 程式：`src-tauri/src/harness/`（mod／root／server／eval／dialog／route／ai_log／helpers.js），AI log 掛點在 `transport/client.rs`、`transport/responses.rs`、`cli/runner.rs`、`cli/install.rs`、`commands/cli_setup.rs`（皆 feature 限定）；`smart_free::pick_primary` 供 `prepare_call` 與 route 預覽共用；`src-tauri/build.rs` 擋反向組合、`src-tauri/tauri.harness.conf.json`；`scripts/harness.mjs`、`scripts/harness-e2e.mjs`、`scripts/check-harness-isolation.mjs`（verify 一步）。
- 用法：`npm run harness:build` → `node scripts/harness.mjs launch --fresh [--config-from FILE] --root <dir>`，不帶參數印用法；所有指令帶同一個 `--root`。合成 Enter 不送出表單，要送出用 `submit`。回歸跑 `node scripts/harness-e2e.mjs <root> <TestCards/WestFantsy.png> <截圖目錄>`（50 步）。
- 驗收：三個 spike 與 iframe 隔離成立；控制埠拒絕規則、跨 root 鎖、重用 root 的 symlink／不可列目錄拒絕；對話窗 UI 實跑；西幻卡端對端 50 步零 AI 派送；route 不錯套退檔；AI log 並行寫讀完整、寫入失敗可偵測、假 CLI spawned／spawn-failed；正式資料遞迴 hash 零差異。verify 全綠。

## 待驗（排在實測佇列）
- 智慧免費真供應商：route 預覽與 `responder` 事件（只在官方 OpenRouter 網址下啟用，本機假端點測不到）。
- 安裝探測與終端安裝腳本的 AI log（要真的跑 CLI 安裝／登入流程）。
- 正式包帶 `TT_HARNESS_ROOT` 不開 listener：需確實隔離資料的環境（獨立 macOS 帳號或 VM）〔模型判斷·未裁決〕。

## 已知限制
- 測試包注入樣式把 CSS 過渡／動畫時長歸零（`harness/motion.rs`）：視窗不可見時 WebKit 凍結動畫時間軸，不歸零會讀到停在起點的樣式與截圖；要看動態本身得另用 test:webkit。卡片介面 iframe 不注入。
- `shot` 截圖底部多 64px（32pt）灰帶：WKWebView 快照含標題列高度，視窗內容完整、未裁切。
- 本機 shell 包裝會擋含字面 `eval`、變數展開成路徑的指令：CLI 用 `js` 別名、路徑寫字面值。
- 鍵盤／焦點回歸屬 menu-keyboard-webkit（`npm run test:webkit`），本通道的 `press` 只派給 app 自己的鍵盤處理器。
- 視窗被別的視窗蓋住時頁面會變 `visibilityState=hidden`，WebKit 凍結 CSS 動畫並停掉 `requestAnimationFrame`，卡片介面 iframe 的淡入動畫／rAF 內容就停在起點、`shot` 只見底色（2026-10-07 實測；iframe 本身拍得到）。已修：`harness/occlusion.rs` 在啟動時對主 WKWebView 呼叫私有 SPI `_setWindowOcclusionDetectionEnabled:NO` 並 post 遮擋狀態通知，被蓋住時仍是 visible、動畫與 rAF 照跑，不搶前景。實測（macOS 26.5.1）：被蓋住時主頁與 iframe 皆 visible、rAF 約 60 次／秒、`shot` 含靜態／動畫／rAF／計時器四項；WestFantsy 卡自帶介面（匯入即有，零 AI）整頁渲染正常；零 AI 更新法：`invoke append_transcript`（scene 取 `read_state` 的 `current_scene`）追一則含 `<GoldenRPG_UI>` 區塊的 GM 旁白，回大廳重進桌，介面換成遊戲殼（srcdoc 192402→212147 字元且含標記、PNG 由選角頁變遊戲頁）。
  - 邊界：縮小視窗、Cmd+H 隱藏時 `isVisible` 本身為 NO，此招無效（實測 hidden、rAF 0）；其他 Space 未驗證。私有 SPI 無相容保證，缺 selector 時 stderr 記一行並維持舊行為，不退回搶前景。
  - 備援繞法（不依賴 SPI）：`js "document.querySelector('iframe.card-interface-frame').srcdoc"` 輸出 `{ok,value}`，`value` 寫成 index.html，`python3 -m http.server` 後用 Browser pane 開（pane 自己被遮住時一樣卡 rAF）。srcdoc 是當下快照，收不到後續推送。
