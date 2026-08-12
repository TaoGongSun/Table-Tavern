# Task
Task-ID: character-to-player-card
Title: 角色卡升級成玩家卡（角色編輯頁的獨立入口）
Status: todo
Created: 2026-08-13T00:30:00.126474+08:00
Updated: 2026-08-13T00:30:00.126474+08:00

## Summary
2026-08-10 立案。桌上任何一張角色卡都應該能在**角色編輯頁**改成玩家卡，與 AI 重構無關——重構面板只在 AI 認定某位是 `{{user}}` 時才問一次（2026-08-10 拍板改成如此），之後玩家改變主意就沒有入口。

現況：玩家卡只能在建卡時指定，或走重構套用時的 `player_card_assigned` 路徑（[receipts.rs:92](../../src-tauri/src/receipts.rs#L92)）。

規格細節（待拍板）見 [plans/character-to-player-card.md](../plans/character-to-player-card.md)。

## Next action
- 2026-08-10 立案；重構面板只在 AI 認人時問一次，之後改主意需要這條路，兩項待拍板（已有玩家卡時換不換、能不能反向取消）

## Constraints
- 沿用一桌一張玩家卡限制，不因新入口放寬。
