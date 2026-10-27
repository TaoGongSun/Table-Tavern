# takeover-list-structured — 接管桌清單欄改成結構化、GM 只報變動值

## Summary
接管桌「在場人物」這類清單欄目前是一整串 YAML 字串存在單一葉子，GM 每輪重寫整包，鍵名字形會漂移（takeover-yaml-key-glyph 以寫入時校正處理）。改成樹上每人一個物件、GM 只用路徑報有變動的欄位、YAML 由 app 組出，鍵就成為樹路徑、可直接套既有字形折疊，也符合「重構目的＝省模型輸出」。takeover-yaml-key-glyph 做法 D，規模大故另案。

## Next action
未排程；需作者決定要不要做（動重構契約、殼的填法、舊桌搬遷）。

## 驗收方向
GM 每輪輸出只含變動值；介面殼照原卡 YAML 呈現；舊桌搬遷不掉資料。
