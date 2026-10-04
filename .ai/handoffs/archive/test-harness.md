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
