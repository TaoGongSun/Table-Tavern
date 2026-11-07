# Task
Task-ID: gm-relay-unify
Title: GM 接力規則單人版（useChatController）與房間版（room/run.rs）兩份實作合一
Status: done（2026-11-08 合 main，等作者實機）
Created: 2026-11-07
案別：大案（核心估 450–600）；碰到玩家句收回與停止／回合鎖，方案與驗收都送審。

## Summary
同一套接力規則（旁白→點名接話→停止條件）前端單人與後端房間各寫一份，改一邊就要記得改另一邊。合成後端一份驅動，兩條路都用；玩家看得到的行為不變。

## 狀態（2026-11-08 結案）
- 兩包合進 main：單人與房間都走後端 `chat_turn::relay::drive`，前端只剩串流顯示、點名鈕、失敗彈窗與收回玩家句。細節與已知限制見 [plans/gm-relay-unify.md](../plans/gm-relay-unify.md) §5。
- 等作者實機：單人一般送出／讓 X 接話／送出並接話／GM 推進／推進中停止各玩一次；房間雙端推進一輪＋停止（實測佇列梯 2 第 32 項）。
