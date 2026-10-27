# gm-reprints-retired-statusbar — 重構淘汰狀態欄後，GM 仍每回合重印卡的 <details> 狀態欄

## Summary
card-field-overlaps-fixed-keys 真重構實測（clean-orc、characters 模式、claude CLI Sonnet）：重構已把狀態欄格式段淘汰，GM 仍每回合照開場白的樣子重印卡的 `<details>` 狀態欄，多花輸出，且重印值與固定鍵寫法不一（如「食堂老廚子（配角）」）。推測是模仿逐字稿裡的開場白。與「重構目的＝省模型輸出」相悖。

## 已知線索
- 實測經過見 [card-field-overlaps-fixed-keys 交接檔](../handoffs/archive/card-field-overlaps-fixed-keys.md)。
- 方向待查：開場白進上下文時是否剝掉狀態欄段、或指示明說不必重印；要看頻率與模型差異。

## Next action
未排程；先用兩三個模型重現、量每回合多出的輸出。

## 驗收方向
重構後續聊，GM 不再重印已淘汰的狀態欄，開場白顯示不受影響。
