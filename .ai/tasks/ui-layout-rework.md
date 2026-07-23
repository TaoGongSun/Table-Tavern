# Task
Task-ID: ui-layout-rework
Title: 版面重構：角色卡移左側欄＋桌列表可摺疊
Status: todo
Created: 2026-07-23T10:20:00+08:00
Updated: 2026-07-23T10:20:00+08:00

## Summary
依 NewPlan §9.4（2026-07-23 拍板）：左側欄改以角色卡為主體（直排頭像＋名稱，預留角色圖片位），移除聊天區上方的水平 cast-row；已開的桌收進側欄可摺疊區塊。純前端搬位置，不動後端。此任務決定設定鈕與角色圖片的落點，須先於 ui-settings-panel 與 post-mvp-st-import 的圖片顯示動工。

## Next action
- 在 App.tsx 把 cast-row 改成側欄直排角色卡（沿用 Avatar 與選定發言邏輯，建卡表單一併搬入），桌列表包進可摺疊區塊（摺疊狀態存 localStorage）

## Constraints
純視覺狀態（摺疊、側欄寬度）存 localStorage，不進 config；不得移除既有功能（點名發言、建卡、桌改名、側欄拖曳調寬）。
