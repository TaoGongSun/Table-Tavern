# room-seat-delegate — 多人房間階段三：AI 代管座位、到場整理、手機房主暫停

案別：大案，核心估約 350 行（上限 740）。方案 [plans/room-seat-delegate.md](../plans/room-seat-delegate.md) 已定案（Fable 常識審、Sol 方案審 2 輪；暫停改走 `ctl pause` 是常識審查員複核後主線採用）。剩 1 題需作者：手機房主改世界書，建議另立案（方案 §9 W1）。

## 現況
- 分支 `room-seat-delegate`。合併順序：room-player-cards → room-host-actions → 本案。本案最後進 main，送驗收前先 rebase、重跑 verify。
- 包 1（伺服器）、包 2（執行端＋桌面遠端暫停通知）已施工。
- 包 3（手機與房間元件）等 room-host-actions 包 1 把 HostPanel 定型後再做。

## 等 rebase 才補的
- 到 room-player-cards 之後：`invite.current`（§3.6）重用 `live.cardReqs`。
- 到 room-host-actions 之後：
  - `round.undo` edit 模式把 `returns` 放回 `s.returning`。
  - 重生沿用原派工快照（§3.1 必改 5）。
  - 補一條測試：遠端暫停後重開，同步閘門照常打開。

## 不部署、不打包
伺服器改了協定，要先手動部署伺服器再發新 app；部署與打包都等作者說了才做。
