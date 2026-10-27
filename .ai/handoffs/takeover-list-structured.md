# takeover-list-structured — 接管桌清單欄改成結構化、GM 只報變動值

## Summary
接管桌「在場人物」這類清單欄目前是一整串 YAML 字串存在單一葉子，GM 每輪重寫整包、鍵名字形會漂移。改成樹上每個項目一個物件（具名集合）或陣列（序列集合），項目格式寫進骨架的 Mustache 區段，GM 只用路徑報變動欄位，list_keys 整組刪除。作者裁決 2026-10-27「takeover-list-structured 做」。

## 現況
- 計畫兩輪審查共識（Opus、Sol、Grok），見 [plans/takeover-list-structured.md](../plans/takeover-list-structured.md)。
- 包 1（前端填殼：`refactor-shell-sections.ts`、狀態欄）、包 3（`Mechanism.collections` 型別、`transport/list_protocol.rs`、「目前狀態」排序與空集合）已驗收通過、留在分支上。
- 包 2（`mechanism/collection.rs`：集合寫入的形狀把關、具名項目與 `_序`、序列補位、改名、寫入邊界；觸發條件 contains 認集合）已在分支上，verify 綠，待驗收。

## Next action
包 2 驗收後做包 4（重構契約、產生描述與補 `_序`、撤銷寫回描述與回歸測試、刪 list_keys）；測試通道要用時先問主線。

## 驗收方向
GM 每輪輸出只含變動值；介面殼照原卡 YAML 呈現。
