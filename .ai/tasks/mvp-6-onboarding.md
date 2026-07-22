# Task
Task-ID: mvp-6-onboarding
Title: MVP 切片 6：Onboarding（BYOK 引導）
Status: completed
Created: 2026-07-18T22:55:00.519014+08:00
Updated: 2026-07-22T23:10:00+08:00

## Summary
依 NewPlan §4.1＋§9.3：首開零精靈直接落在內建範例桌「迷霧酒館（範例）」（3 角色卡＋開場旁白）；僅 transport=api 且缺 key 時顯示 BYOK 引導（官方頁連結＋費用直覺化＋貼 key 即玩）。實作已提交（4cfcc82）。2026-07-22 使用者 UI 實測兩項通過並結案；過程中修掉 create_sample_world 非冪等與費用文案幣別／數字問題。

## Next action
- 無，已結案。

## Constraints
簡易模式只顯示能力檔位；除 API key 外不得有任何必填欄位。
