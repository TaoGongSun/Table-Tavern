# Task
Task-ID: mvp-6-onboarding
Title: MVP 切片 6：Onboarding（BYOK 引導）
Status: in-progress
Created: 2026-07-18T22:55:00.519014+08:00
Updated: 2026-07-20T01:55:46.809266+08:00

## Summary
依 NewPlan §4.1＋§9.3：首開零精靈直接落在內建範例桌「迷霧酒館（範例）」（3 角色卡＋開場旁白）；僅 transport=api 且缺 key 時顯示 BYOK 引導（官方頁連結＋費用直覺化＋貼 key 即玩）。實作已提交（4cfcc82），cargo test 27 綠、tsc/vite 過；外包 codex（gpt-5.6-sol）實作、主線驗收。

## Next action
- 使用者 UI 實測（搬走 ~/Documents/TableTavern 看首開範例桌；設定切 API 直連看引導面板），通過後結案

## Constraints
簡易模式只顯示能力檔位；除 API key 外不得有任何必填欄位。
