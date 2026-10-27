> 結案 2026-10-27：打字提示移到介面輸入列正上方，三方驗收通過，已併入 main。

# gm-typing-covers-card-date — 「GM 正在打字」提示框蓋住卡片頂列日期

## Summary
卡片介面開著時，「GM 正在打字」提示框蓋住卡片頂列的日期文字。算 bug，要修〔作者裁決 2026-10-27〕。

## 裁決
- 打字提示改放在下方輸入框的正上方〔作者裁決 2026-10-27〕。
- 介面開著時輸入框一直都在：覆蓋層底部固定一條輸入列〔作者裁決 2026-10-11〕，見 [interface-card-panel-v2](../../plans/interface-card-panel-v2.md)；首版見 [interface-card-panel](interface-card-panel.md)。

## 狀態
- 分支 `gm-typing-covers-card-date`；已用測試通道＋本機假 GM 端點重現。
- 做法第 1 輪三方共識，已實作：打字提示移進介面輸入列、排在輸入列正上方；做法、參考、實測見 [plans/gm-typing-covers-card-date](../../plans/gm-typing-covers-card-date.md)。verify 綠。

## Next action
無。

## 驗收方向
介面桌生成中，提示框不遮卡片頂列任何文字、顯示在輸入列正上方；無介面桌的提示框位置不變。
