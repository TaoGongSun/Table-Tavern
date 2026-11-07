# Task
Task-ID: gm-relay-unify
Title: GM 接力規則單人版（useChatController）與房間版（room/run.rs）兩份實作合一
Status: 方案定案（Sol 方案審 2 輪 DONE），包 1 施工中（Grok）
Created: 2026-11-07
案別：大案（核心估 450–600）；碰到玩家句收回與停止／回合鎖，方案與驗收都送審。

## Summary
同一套接力規則（旁白→點名接話→停止條件）前端單人與後端房間各寫一份，改一邊就要記得改另一邊。合成後端一份驅動，兩條路都用；玩家看得到的行為不變。

## 狀態
- 方案：[plans/gm-relay-unify.md](../plans/gm-relay-unify.md)（現況、方向、分包、驗收）。
- 下一步：包 1 Grok 施工 → Sol 驗收；Sol 方案審串 01a1b691（驗收另開包 1 新串）。
