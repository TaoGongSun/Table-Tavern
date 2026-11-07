# Task
Task-ID: ttrpg-combat-tracker
Title: 跑團戰鬥層：先攻順序、HP 與狀態效果由 app 加減、跟骰池綁定（掛在通用狀態欄的角色格上）
Status: todo
Created: 2026-11-07
Updated: 2026-11-07（拆案：通用狀態欄獨立成 table-status-panel，本案瘦身）
案別：中案（核心估 150–250：先攻列、數值加減與事件驗證、骰池結果綁定）。模型回報的事件是信任邊界輸入，不論案別都做方案送審。

## Summary
朋友回饋「有戰鬥時由系統幫忙算數值」。跑團玩家戰鬥中只看三樣：先攻順序、HP 與狀態效果、骰子結果。本案只做這一層：模型回報事件（誰打誰、傷害多少、套了什麼狀態），加減由 app 做，套不上就照常遊戲、面板不動；擲骰維持 app 真隨機底線。容器、預設欄位、預算、一行摘要、多人廣播都在 [table-status-panel](table-status-panel.md)，本案不重做。〔作者裁決 2026-11-07〕

## 依賴
- [table-status-panel](table-status-panel.md)：角色格與數值欄的啟用規則（規則書或骰子記號偵測到時開數字欄）。
- [ttrpg-rules-system](ttrpg-rules-system.md) v1：骰池。

## 要先拍〔模型判斷·未裁決〕
- 先攻順序由誰宣告：GM 回合開場一次排好、app 記住輪到誰；玩家手動調整要不要開放。
- 多人房間的「輪到誰」與既有回合制（房間回合列）怎麼合，不做兩套輪次。
