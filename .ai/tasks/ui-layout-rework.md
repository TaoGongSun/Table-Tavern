# Task
Task-ID: ui-layout-rework
Title: 版面重構：角色卡移左側欄＋桌列表可摺疊
Status: completed
Created: 2026-07-23T10:20:00+08:00
Updated: 2026-07-23T22:40:00+08:00

## Summary
依 NewPlan §9.4（2026-07-23 拍板）：左側欄改以角色卡為主體（直排頭像＋名稱，預留角色圖片位），移除聊天區上方的水平 cast-row；已開的桌收進側欄可摺疊區塊。純前端搬位置，不動後端。此任務決定設定鈕與角色圖片的落點，須先於 ui-settings-panel 與 post-mvp-st-import 的圖片顯示動工。

## Next action
- 無。2026-07-23 使用者視覺驗收通過（截圖示角色卡左欄、桌列表摺疊／展開皆正常），結案

## Constraints
純視覺狀態（摺疊、側欄寬度）存 localStorage，不進 config；不得移除既有功能（點名發言、建卡、桌改名、側欄拖曳調寬）。
