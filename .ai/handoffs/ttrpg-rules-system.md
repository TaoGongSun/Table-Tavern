# Task
Task-ID: ttrpg-rules-system
Title: 跑團規則系統：規則書引入＋擲骰＋角色紙（內建常見開放規則精簡版）
Status: done（v1 2026-11-08 合 main，等作者實機）
Created: 2026-08-13
Updated: 2026-11-08
案別：v1 大案（立案估 200–300，實作核心約 Rust 190＋前端 A 360／B 560／C 135）。

## Summary
跑團是聊天桌上的能力組，不是獨立桌種〔作者裁決 2026-11-07〕。v1 交付：GM 每回合預餵 32 顆真隨機骰池、敘事標骰號 `(d20#2=19)`、app 白話對帳；單人骰鈕（GM 骰式一鍵＋自訂、空白擲完續敘、有草稿等送出）；房間 GM 代擲（房間骰鈕留 v2）；桌設定開關與匯入偵測；內建 5e SRD 5.1 精簡裁決版與選書卡〔作者裁決 2026-11-08〕。拍板、技術方案、注入實測與已知限制見 [plans/ttrpg-rules-system.md](../plans/ttrpg-rules-system.md)。

## 相鄰案
- 戰鬥數值（先攻、HP 加減、骰池綁定）歸 [ttrpg-combat-tracker](../tasks/ttrpg-combat-tracker.md)，本案 v1 已是它的地基。
- 其他內建規則書各一小案：ttrpg-builtin-pf2e／-dungeon-world／-cairn／-brp（見 BACKLOG）。

## 下一步
- 作者實機（實測佇列梯 2 第 33 項）。
- v2 候補：房間玩家骰鈕（含手機）、角色紙結構化、嚴格模式（兩段式喊骰）、AI 整理精靈。
