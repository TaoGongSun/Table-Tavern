# Task
Task-ID: release-2-ci-windows
Title: 發佈 2：CI 產線＋Windows 安裝檔（tauri-action）
Status: todo
Created: 2026-07-22T22:35:12.109145+08:00
Updated: 2026-07-22T22:35:12.109145+08:00

## Summary
tauri-action 單一 workflow 出 macOS 與 Windows 產物，發到 GitHub release（NewPlan §16.3–16.4）。2026-07-24 使用者調整優先序：**Windows 玩家基數大，Windows 簽章優先於 Mac**，不再等 release-1；原拍板「Windows 不買憑證」待報價查證後重新拍板（候選便宜路線：Certum 開源憑證、Azure Trusted Signing 月費制——價格與資格皆未查證，禁當事實引用）。NSIS 設定 WebView2 缺少時自動安裝。

## Next action
- 先查證 Windows 簽章便宜路線的現行價格與申請資格（Certum 開源／Azure Trusted Signing），給使用者拍板買不買
- 拍板後寫 tauri-action workflow；Mac 簽章材料等 release-1 就緒再併入同一條產線

## Constraints
單一 build，無免費／收費雙版本；自動更新 v1 不做，更新靠重新下載 release；發布帖須透明聲明資料流向（NewPlan §16.4）。
