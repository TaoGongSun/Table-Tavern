# test-harness 交接

測試建置專用的控制通道，讓主線不靠使用者點擊對真 app 跑 GUI 實測。方向〔作者裁決 2026-10-02〕；規格與 spike 結果見 [plans/test-harness.md](../plans/test-harness.md)。分支 `test-harness`。

## 現況：包 1 完成（2026-10-02）
- 程式：`src-tauri/src/harness/`（mod／root／server／eval／dialog）、`src-tauri/build.rs` 反向組合檢查、`src-tauri/tauri.harness.conf.json`、`scripts/harness.mjs`、`scripts/check-harness-isolation.mjs`；更新器 `update_install`／`rollback_install` 回錯、`update_post_launch` 空轉。前端零改動。
- 用法：`npm run harness:build` → `node scripts/harness.mjs launch --fresh --root <dir>`；之後 `js`（＝eval）、`dialogs`／`dialog-wait`／`answer`、`shot`、`status`、`quit`，都帶同一個 `--root`（或設 `TT_HARNESS_ROOT`）。
- 實跑驗收（全在 scratchpad 測試根目錄）：
  - eval 回值、拋錯帶訊息、DOM 節點／循環各回明確錯誤；錯 token 401、帶 Origin 403、Host 為 localhost 403、Transfer-Encoding 400、重複 Host 400；harness.json 與 root 為 0600／0700，只在 127.0.0.1 監聽。
  - 三個 spike 與 iframe 隔離都成立（證據在計畫檔）。
  - 跨 root 鎖：A 在跑時 `launch --fresh --root B` 以 78 結束、A 的 localStorage 不受影響；外部持鎖時 B 取不到，放鎖後 B fresh 啟動成功且 sentinel 被清空。
  - 危險 root（正式資料內、家目錄）、缺 `TT_HARNESS_ROOT`、feature 配正式 identifier 都以 78 拒絕；harness 設定未開 feature 時 build.rs 讓建置失敗。
  - 正式包（先建 harness 包再建）：identifier `com.tabletavern.app`，執行檔不含 `harness-reply`／`TT_HARNESS_ROOT`／`test-harness`／`takeSnapshotWithConfiguration`（靜態佐證）。「正式包帶 `TT_HARNESS_ROOT` 也不開 listener」**待驗**：依約不啟動正式 app，尚無動態證據。
  - 正式資料快照：`~/Library/Application Support/TableTavern/grok-home` 與 `model_catalog.json` 在 20:03／20:23 的寫入，使用者 2026-10-02 確認那段時間自己開了正式 app（`npm run tauri dev` 或打包版），歸因於此。測試包未寫正式資料的動態證據以測試期間 `~/Documents/TableTavern` 零差異、測試包的 grok 子程序（22056、29171）都寫在測試根目錄為準。
  - 重用 root 的防逃逸：持鎖後、任何寫入前整棵 root 不得有 symlink，data／config 與 config.json 目的地 canonical 後須在 root 內且不碰受保護路徑（`root::prepare`，含 data symlink、config.json symlink、深層 symlink 的反向測試）。

## 下一步
1. 主線檢查與送審包 1。
2. 包 2：`file` 指令＋對話窗 UI 實跑；包 3：DOM 指令、route、AI log、端對端驗收。

## 約束
- 不開正式版 Table Tavern、不碰正式資料、不燒 AI 額度；鍵盤／焦點回歸屬 menu-keyboard-webkit 案。
- 本機 shell 包裝會擋含字面 `eval`、變數展開成路徑的指令：CLI 用 `js` 別名、路徑寫字面值。
