# Task
Task-ID: post-mvp-st-import
Title: MVP 後第一優先：SillyTavern 角色卡匯入
Status: in-progress
Created: 2026-07-18T22:55:00.699921+08:00
Updated: 2026-07-23T15:45:00+08:00

## Summary
依 NewPlan §5.2：支援 V2 card spec（內嵌 JSON 的 PNG 或純 JSON）。欄位對應：name/description/personality/scenario→角色卡對應欄；first_mes、mes_example→開場白與語氣範例；character book→角色私有資訊。對不上的欄位保留原始資料不丟棄。只做匯入不做匯出。2026-07-23 增補：匯入時把 PNG 原圖存進該世界目錄，並做角色圖片顯示／隱藏開關（預設顯示；無圖手建卡維持 emoji），顯示位置在左側欄角色卡（NewPlan §9.4），故 UI 部分等 ui-layout-rework 完成後接。

## Next action
- 後端切片完成（V2/V1 解析＋欄位對應＋import_character command＋測試 36 綠，見 handoffs/post-mvp-st-import.md）；剩前端匯入鈕＋角色圖顯示／隱藏開關，等 ui-layout-rework 視覺驗收後接

## Constraints
不提前實作（KICKOFF §6）；不做反向匯出相容（NewPlan §5.2）。
