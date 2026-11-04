# room-host-actions — 多人房間房主操作

案別：大案，核心估約 620 行（上限 850）。規格、分包、驗收在 [plans/room-host-actions.md](../plans/room-host-actions.md)（Sol 方案審 2 輪定案）。

## 現況

- 分支 `room-host-actions`（已 rebase 到 main）。包 1（GM 鈕）、包 2（收回／重生）已實作、verify 綠，待 Sol 驗收；包 3（重開補送）要改 `link.rs::welcome`，等階段二 `room-player-cards` 包 2 落地再做。
- 合併順序：room-player-cards → room-host-actions → room-seat-delegate；送驗收前先 rebase 到 main、重跑 verify。
- 重生沿用原派工快照（players、arrivals、departures、returns），見 room-seat-delegate 方案 §3.1。

## 下一步

- 包 1、包 2 逐包送 Sol 驗收；包 3 等交辦。
- 已知限制〔模型判斷·未裁決〕：收回改字後，桌上在場名單（`set_party`）要到下一個派工才歸位；往回翻頁跨 200 筆邊界時，已收回的段落可能被翻回來。
