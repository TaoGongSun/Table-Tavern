# takeover-list-structured — 接管桌清單欄改成結構化、GM 只報變動值

## Summary
接管桌「在場人物」這類清單欄目前是一整串 YAML 字串存在單一葉子，GM 每輪重寫整包、鍵名字形會漂移。改成樹上每個項目一個物件（具名集合）或陣列（序列集合），項目格式寫進骨架的 Mustache 區段，GM 只用路徑報變動欄位，list_keys 整組刪除。作者裁決 2026-10-27「takeover-list-structured 做」。

## 現況
- 分支 `takeover-list-structured` 已開；計畫檔 [plans/takeover-list-structured.md](../plans/takeover-list-structured.md) 寫好，等主線送 Sol／Grok 審。
- 尚未動程式碼。

## Next action
審查共識後依計畫分包逐包做；包 2、包 4 與 takeover-flat-nonfixed-keys 檔案重疊，開工前由主線排先後。

## 驗收方向
GM 每輪輸出只含變動值；介面殼照原卡 YAML 呈現。
