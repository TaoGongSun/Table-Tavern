# Task
Task-ID: ui-settings-panel
Title: 設定按鈕：外觀偏好（文字大小等）
Status: todo
Created: 2026-07-23T00:15:00+08:00
Updated: 2026-07-23T00:15:00+08:00

## Summary
新增一個明顯的「設定」入口，收納非 AI 類的使用偏好：至少含文字大小調整；語言下拉（ui-i18n-switch 已做，暫放側欄底部）屆時一併移入。與既有「AI 設定」（key／CLI／檔位）分開或分頁呈現，避免一般使用者被 AI 參數嚇到。

## Next action
- 定 UI 位置（側欄底部齒輪鈕？）與第一批偏好項目：文字大小（存 config.preferences，套 CSS 變數）、語言（搬移現有下拉）；再看是否納入佈景主題（與 release-4-theme-pack 銜接）

## Constraints
偏好存 config.preferences（跨裝置語意清楚的 key）；純視覺狀態（如側欄寬度）維持 localStorage，不進 config。
