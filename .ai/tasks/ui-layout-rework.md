# Task
Task-ID: ui-layout-rework
Title: 版面重構：角色卡移左側欄＋桌列表可摺疊
Status: in-progress
Created: 2026-07-23T10:20:00+08:00
Updated: 2026-07-23T15:05:00+08:00

## Summary
依 NewPlan §9.4（2026-07-23 拍板）：左側欄改以角色卡為主體（直排頭像＋名稱，預留角色圖片位），移除聊天區上方的水平 cast-row；已開的桌收進側欄可摺疊區塊。純前端搬位置，不動後端。此任務決定設定鈕與角色圖片的落點，須先於 ui-settings-panel 與 post-mvp-st-import 的圖片顯示動工。

## Next action
- 程式碼已完成並通過建置與功能核對（見 handoffs/ui-layout-rework.md），剩使用者在真實 App 視覺驗收（摺疊狀態重啟保留、角色卡選中視覺、窄寬側欄不破版），通過即結案

## Constraints
純視覺狀態（摺疊、側欄寬度）存 localStorage，不進 config；不得移除既有功能（點名發言、建卡、桌改名、側欄拖曳調寬）。
