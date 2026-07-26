# Task
Task-ID: cli-connected-badge
Title: CLI 已連結狀態記憶：按鈕換「已連結 ✓」＋不重發登入
Status: in_progress
Created: 2026-07-26T12:00:00+08:00
Updated: 2026-07-26T12:00:00+08:00

## Summary
2026-07-26 使用者實測回報：已登入的 CLI 按「登入／驗證」仍會重發登入請求，希望已連結就藏按鈕。拍板做法：app 以 `preferences["cli_connected:<id>"]` 記住連結狀態——安裝／驗證流程 done→true、error→false，CLI 實聊成功一輪也自動標 true；設定頁該供應商列改顯示「已連結 ✓」＋小顆「重新驗證」（token 過期時的重驗入口），未連結維持原按鈕。Windows 端 agy／grok 的 `pre_probe` 維持 false（未登入時探針會觸發 OAuth 副作用，install.rs 有註解，codex 曾誤翻已退回）；Mac 腳本本就先探針成功即跳過登入，無需改。

## Next action
- 使用者實測（DMG 0.2.0 第二版）：已登入的 CLI 實聊一輪後回設定頁，該列應變「已連結 ✓」；按「重新驗證」不應再彈登入。通過即結案

## Constraints
agy／grok 未登入時禁止無頭執行探針（OAuth 副作用）；已知限制：Windows 上 agy／grok 已登入但從未在 app 內驗證／實聊過者，首次按鈕仍走登入視窗，實聊一次即補上旗標。
