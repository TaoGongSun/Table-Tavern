# room-small-fixes — 房間三小事

Status: 已合 main（2026-11-08），等實機
Created: 2026-11-08
案別：大案。立案估小案，實作核心新增約 444 行（含 CSS 與重構搬移）。第 3 件碰身分（信任邊界），依規定做方案送審。規格見 [plans/room-small-fixes.md](../plans/room-small-fixes.md)。

## Summary
朋友實機測房間容易碰到的三件：選角清單簡介露 Markdown 原文（客戶端剝成純文字）；手機看不到「某某觀望」（故事串補觀望行）；換裝置名單留兩列（玩家按「這是我的？」、房主確認，伺服器一個訊息移座＋移除舊列）。

## 下一步
- 要先手動部署房間伺服器（舊伺服器不認 `seat.request`），部署與打包等作者說。
- 部署後照[實測佇列](../reference/verification-queue.md)梯 3 的 room-small-fixes 那列實機驗。
