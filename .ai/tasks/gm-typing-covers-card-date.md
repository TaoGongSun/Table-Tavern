# gm-typing-covers-card-date — 「GM 正在打字」提示框蓋住卡片頂列日期

## Summary
卡片介面開著時，「GM 正在打字」提示框蓋住卡片頂列的日期文字。算 bug，要修〔作者裁決 2026-10-27〕。

## 裁決
- 打字提示改放在下方輸入框的正上方〔作者裁決 2026-10-27〕。
- 介面開著時輸入框一直都在：覆蓋層底部固定一條輸入列〔作者裁決 2026-10-11〕，見 [interface-card-panel-v2](../plans/interface-card-panel-v2.md)；首版見 [interface-card-panel](../handoffs/archive/interface-card-panel.md)。

## 已知線索
- 證據截圖：`/private/tmp/claude-501/-Users-<user>--claude-spares-spare-yfWgps/853b218d-0b3d-4f00-b3b6-0b81bbbd6996/scratchpad/sd/shots/18c-streaming-1.png`（暫存檔，可能已被清掉；清掉就重現一次）。
- 其餘未查。

## Next action
未排程；開工先重現，再找提示框定位（目標位置＝介面底部輸入列正上方）與卡片 iframe 的重疊關係。

## 驗收方向
介面桌生成中，提示框不遮卡片頂列任何文字、顯示在輸入列正上方；無介面桌的提示框位置不變。
