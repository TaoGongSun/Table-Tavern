# Task
Task-ID: cli-auto-connect
Title: CLI 自動連接：背景偵測＋登入跳轉自動回
Status: todo
Created: 2026-07-23T00:15:00+08:00
Updated: 2026-07-23T00:15:00+08:00

## Summary
目標是把 CLI 訂閱模式的接入便利度最大化：App 在背景自動偵測／連接已安裝的 CLI，需要瀏覽器登入（OAuth）時自動開網頁，完成後自動跳回 App 繼續，使用者不需手動操作終端機。現況：detect_clis 只做被動偵測，未登入的 CLI 顯示「未偵測到；請自行安裝並登入」，登入流程完全交給使用者。

## Next action
- 先查證各 CLI（claude、codex）的無頭登入／token 狀態檢查介面：能否從 App 觸發登入流程、登入完成如何得知（輪詢？callback？），再定 UX 流程（風險告知仍必須前置，NewPlan §4.2）

## Constraints
CLI 訂閱模式的風險告知勾選不可被自動化流程繞過；不代管、不觸碰使用者憑證內容。
