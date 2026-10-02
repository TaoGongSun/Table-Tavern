# backend-zh-messages — 後端寫死繁中訊息改回傳代碼

分支 `backend-zh-messages`（worktree：`../Table-Tavern-backend-zh-messages`，從 main 分出）。規格與盤點見 [plans/backend-zh-messages.md](../plans/backend-zh-messages.md)。

## 狀態

- 需修復整頁說明（`src-tauri/src/data/format/commit.rs` 的 `REPAIR_*` 四則）是寫死繁中，非中文介面照樣顯示中文。本案改成後端回傳原因代碼、前端 i18n 顯示。
- 範圍：A＋B＋C 類〔作者裁決 2026-10-02〕；D 類另立案。做法與分包在計畫檔，已經 Sol 審過；Grok 額度用完，本案只經 Sol 審與驗收〔作者裁決 2026-10-02〕。

## 下一步

1. 逐包派子代理實作（包 1 開始） → 每包 `npm run verify` 全綠 → Sol 驗收。
