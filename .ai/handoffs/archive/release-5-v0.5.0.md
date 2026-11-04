# release-5-v0.5.0 — 桌面版第三個正式版 0.5.0（多人連線房間、視覺小說模式）

> 結案 2026-11-04：v0.5.0 已公開（https://github.com/TaoGongSun/Table-Tavern/releases/tag/v0.5.0 ，標 Latest、8 個產物齊，`releases/latest/download/` 的 `latest.json`（0.5.0）與兩個固定檔名都 200）；網頁版 `table-tavern.pages.dev` 推 main 自動部署 1fe3b35f 成功；房間伺服器先部署 bdab3bc7。小案（核心 0 行）；Sol 驗收 2 輪（必改：發佈說明的資料去向寫錯，已修），常識審查白話改寫。第一次正式 run 的 finalize 沒被 GitHub 排進去、也不能重跑，刪 tag 在同一 commit 重推後通過。

## 拍板〔作者裁決 2026-11-04〕
- 0.5.0 前的多人房間待辦都進 main 後發 0.5.0；流程沿用 [plans/release-2-ci-windows.md](../../plans/release-2-ci-windows.md)。
- 不做 0.4.0 舊桌升級實測：沒有在玩舊桌的玩家；CHANGELOG 不寫舊桌相容說法。
- 多人房間說明書（room-guide）做完才發，桌面開房對話框入口進 0.5.0。

## 剩下
- 作者兩平台實機下載安裝能開；0.4.0→0.5.0 App 內更新由作者在裝著 0.4.0 的機器上看（[desktop-update-detect](../desktop-update-detect.md)）。
