# Task
Task-ID: character-to-player-card
Title: 角色卡升級成玩家卡（角色編輯頁的獨立入口）
Created: 2026-08-10T22:45:00+08:00
Updated: 2026-08-10T22:45:00+08:00
Status: todo

## Summary
2026-08-10 立案。桌上任何一張角色卡都應該能在**角色編輯頁**改成玩家卡，與 AI 重構無關——重構面板只在 AI 認定某位是 `{{user}}` 時才問一次（2026-08-10 拍板改成如此），之後玩家改變主意就沒有入口。

現況：玩家卡只能在建卡時指定，或走重構套用時的 `player_card_assigned` 路徑（[receipts.rs:92](../../src-tauri/src/receipts.rs#L92)）。

## 待拍板
1. 一桌一張玩家卡的既有限制怎麼處理：桌上已有玩家卡時，是換過去（原本那張退回普通角色卡）還是擋下來要玩家先取消？
2. 要不要能反向取消（玩家卡退回普通角色卡）。

## Next action
未排程。動工點：角色編輯頁按鈕列（照 [ui-action-button-placement 慣例](../../CLAUDE.md) 置頂）加入口，後端沿用既有 `player_card_id` 寫入路徑。

## Constraints
- 沿用一桌一張玩家卡限制，不因新入口放寬。
