# card-interface-toolbar-overlap — 卡片介面右上「換幕／關閉」工具列浮蓋卡片

## Summary
卡片介面覆蓋層右上的「換幕／關閉」工具列用浮動定位疊在 iframe 上，會壓到卡片自己的右上按鈕（NorthHall 的齒輪被擠到邊）。與 gm-typing-covers-card-date 同型（浮在卡片內容上），該案只處理打字提示，故另立。

## 已知線索
- 出處：gm-typing-covers-card-date 重現時發現；截圖 `scratchpad/typing-run/shots/02-streaming-before.png`（暫存檔，可能已清）。
- 介面顯示問題先參考 Marinara／SillyTavern 怎麼放卡片外的控制項，只參考不模仿〔作者裁決 2026-10-27〕。

## Next action
未排程；開工先重現，再決定工具列改佔版面還是移到輸入列一側。

## 驗收方向
介面開著時，卡片頂列與右上按鈕不被 app 控制項遮住。
