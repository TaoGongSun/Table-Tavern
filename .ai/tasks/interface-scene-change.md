# Task
Task-ID: interface-scene-change
Title: 介面桌換幕：前情提要進介面正文槽、面板與狀態樹原樣續存
Status: todo

## Summary
機制複雜輸出長的介面卡（Transfur 型）沒有換幕配套就不敢長玩。目標：換幕後介面照常運作，【前情提要】顯示在正文槽，其餘面板與數值完整保留，後續回合照舊走 patch 契約。缺口比預想小：狀態樹權威在 state.json、換幕不動它，GM 每輪狀態注入跨幕存活。Sol 第 1 輪覆核（2026-08-14）已併入：recap 防劫持改雙層（事件帶 origin、渲染跳過 direct-first 必走正文槽為主層，提示詞禁標記為第二層）；換幕鈕放 CardInterfaceOverlay 工具列、不加 postMessage 橋；驗收拆兩條獨立案例（換幕當下驗重寫／退回，續玩兩回合另驗 patch——原稿順序會被守門擋下）；分岔明訂＝樹回到來源幕最後快照。

設計底稿見 [plans/interface-scene-change.md](../plans/interface-scene-change.md)，含兩個【待實測】假設與四包草案。

## Next action
- 開工首步＝在西幻接管桌實測兩個【待實測】假設（換幕後檯面樹不變、前情提要落正文槽），結果回填底稿再分包

## Constraints
- 只服務介面優先軌；角色優先軌用現行換幕即可。
- 換幕不得影響介面運作：面板數值不歸零、格式不崩是硬驗收；驗收同時比對 state.json 與畫面。
- recap 主防線在渲染層（origin 跳過 direct-first），提示詞約束只當第二層。
- Transfur 型驗收排在 interface-takeover-spike 待辦 2（逐型驗卡）之後；西幻接管桌可先驗。
