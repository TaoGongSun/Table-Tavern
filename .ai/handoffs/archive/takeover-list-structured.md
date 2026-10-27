> 結案 2026-10-28：四包 Sol、Grok 驗收 DONE，真機驗收通過，整理成一包一筆進 main。

# takeover-list-structured — 接管桌清單欄改成結構化、GM 只報變動值

## Summary
接管桌「在場人物」這類清單欄原本是一整串 YAML 字串，GM 每輪重寫整包、鍵名字形會漂移。改成「集合」：具名集合以名字為鍵（項目是物件、帶 app 管的 `_序`），序列是 0..n-1；項目格式寫在骨架的 Mustache 區段 `{{#集合}}…{{/集合}}`，GM 只用路徑報變動欄位；list_keys 整組刪除。作者裁決 2026-10-27「takeover-list-structured 做」。規格與已知限制見 [plans/takeover-list-structured.md](../../plans/takeover-list-structured.md)。

## 結果
- 包 1 前端填殼、包 2 `mechanism/collection.rs` 寫入、包 3 `transport/list_protocol.rs` 與「目前狀態」、包 4 重構契約／驗證／套用／撤銷與刪 list_keys。
- 真機驗收：北境驿站 sonnet 重構新產物、假 GM 七回合、Haiku 續聊 5 回合，更新區塊平均 1426→839 字元（約 -41%）；細節與已知限制見 [計畫檔](../../plans/takeover-list-structured.md)。

## 驗收方向
GM 每輪輸出只含變動值；介面殼照原卡 YAML 呈現。
