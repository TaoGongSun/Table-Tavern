# Task
Task-ID: sample-world-i18n
Title: 範例桌內容依語系產生（首開先選語言）
Status: in-progress
Created: 2026-07-23T00:40:00+08:00
Updated: 2026-07-24T00:05:00+08:00

## Summary
首開自動建立的範例桌內容依語系產生。順序問題 2026-07-23 使用者拍板：**首開先跳語言選擇畫面（下拉，預選跟系統語系）**，選完才建對應語言的範例桌——理由：外語使用者首開看到中文介面容易嚇退。已存在的桌是資料，不回頭改。

## Next action
- 程式碼完成（create_sample_world 依 lang 產 zh-TW／en 內容＋前端 FirstRun 畫面，cargo test 41 綠＋npm build 綠，見 handoffs/sample-world-i18n.md）；剩使用者實測首開流程（清掉 worlds 目錄與 config 語言偏好模擬首開）即結案

## Constraints
只影響新建的範例桌；使用者已動過的桌一律不改。後端錯誤訊息的 i18n 不在本任務範圍。介面擴充到更多語言另立 i18n-more-languages。
