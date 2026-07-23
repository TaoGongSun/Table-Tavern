# Task
Task-ID: ui-settings-panel
Title: 設定按鈕：外觀偏好（文字大小等）
Status: completed
Created: 2026-07-23T00:15:00+08:00
Updated: 2026-07-24T00:40:00+08:00

## Summary
依 NewPlan §9.4（2026-07-23 拍板）：單一設定鈕開設定視窗，內部分頁——外觀偏好（預設頁：文字大小、語言）／AI 連線（key、傳輸層、檔位模型）。原側欄底部的 AI 設定摺疊區與語言下拉一併移入。等 ui-layout-rework 定出新版面後開工。

## Next action
- 無。2026-07-24 使用者複驗五檔文字大小通過（附截圖），結案

## Constraints
偏好存 config.preferences（跨裝置語意清楚的 key）；純視覺狀態（如側欄寬度）維持 localStorage，不進 config。
