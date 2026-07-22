# Task
Task-ID: mvp-4-director
Title: MVP 切片 4：簡易導演（GM）
Status: in-progress
Created: 2026-07-18T22:55:00.338239+08:00
Updated: 2026-07-20T01:44:16.984790+08:00

## Summary
依 NewPlan §6.1／§7.0：GM 上下文＝world.md（只進 GM）＋全部公開歷史；GM 可選下一位發言者、插入旁白；控制每回合最大發言數。實作已完成並提交（985ce42），cargo test 26 綠、tsc/vite 過、真實 claude CLI 冒煙通過（點名解析正確、旁白扣題）。

## Next action
- 使用者開 App 實測「GM 旁白」與「GM 推進」兩鈕，通過後 handoff complete＋task complete

## Constraints
不提前做完整 GM 模式（骰子、戰鬥、地圖，NewPlan §6.2／§12）。
