# shell-keep-old-on-timeout — 新殼逾時沒載好就保留舊殼，不翻成空白

## Summary
實測 4h（2026-10-28）：卡片介面殼靠 CDN（Vue、tailwind、字型）載入，CDN 卡住時新殼在 1500 ms 後被強制翻上來，Vue 卡整片空白；宿主介面與下一回合照常。酒館同情況也是空白、無備援；Marinara 不跑卡片腳本。作者裁決：逾時時保留上一回合已畫好的殼、只解鎖不翻面，新殼之後載好再翻〔作者裁決 2026-10-28〕。設計背景見 [shell-update-flash](../handoffs/archive/shell-update-flash.md)。

## Next action
開工。

## Constraints
- 逾時仍要解鎖，宿主介面與下一回合不得被卡住。
- 第一次開桌就卡住（沒有舊殼）時維持現行行為。
- 已知代價：卡住期間殼上數值停在上一回合（app 自己的狀態列是新的）。
