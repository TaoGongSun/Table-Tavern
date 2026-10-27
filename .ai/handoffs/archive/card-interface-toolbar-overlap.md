# card-interface-toolbar-overlap — 卡片介面右上「換幕／關閉」工具列浮蓋卡片右上按鈕

分支 `card-interface-toolbar-overlap`。做法與參考見 [plans/card-interface-toolbar-overlap.md](../plans/card-interface-toolbar-overlap.md)。

> 結案 2026-10-27：squash 進 main，Sol、Grok 驗收 DONE。

## 現況
- 已重現（測試通道，NorthHall 卡介面外殼）：工具列以 `position:absolute; top/right:0.6rem` 浮在 iframe 上，壓住卡片頂列右側與右上設定鈕區。
- 第 1 輪送審 Sol、Grok DONE（40px 常駐可接受）；已實作（工具列改頂列、CSS 一段、測試加結構鎖一案）、verify 綠、測試通道實測完成，等驗收。

## 下一步
- 送驗收 → 共識後結案（squash 進 main、刪分支）。

## 驗收方向
介面開著時，卡片頂列與右上按鈕不被 app 控制項遮住。
