# room-host-bar-observations：房間回合列三個小問題

案別：小案（核心 38 行，測試 59 行）。分支 `room-host-bar-observations`，施工與實測完成，等主線送審、結案合 main。

## 做法（已實作）
1. **桌面「⋯」浮層蓋住換幕**：原本 `popover = open || handoff !== null`，交給手機的連結一出來浮層就關不掉。改成只看 `open`，點外面、Esc、再按 ⋯ 都能收；連結留在狀態裡，再打開還在。手機配對成房主後（這台不再是房主）連結不再顯示。`src/features/room/DesktopRoom.tsx`。
2. **回合數跳號**：伺服器 `roundNo` 是只增不減的流水號（開場、GM 鈕、收回後重開、重生都會 +1），同時是故事段、收回、各動作的識別鍵，不動它。只改顯示：`shownRound()`（`party/src/features/room/room-state.ts`）＝本幕（最後一段幕摘要之後）有玩家行動列的回合數，當前回合還沒算進去再 +1。擲果列也算（只擲骰沒寫字的回合），但有收回提示（段號 33）的回合不算；即時事件與重連快照算出同一個數。結果：GM 鈕不跳號、收回改字回到同一輪、**每幕從第 1 回合算起**（快照只帶本幕，全域連續要改伺服器）。
3. **手機標頭回合數被截**：摘要鈕改成 flex 換行，狀態句與「第 N 幕 · 第 M 回合」各自不拆；一行放不下時回合號整段換到下一行；拿掉中間的「 · 」改用間距（中文 iPhone 寬度因此一行放得下）。讀舊劇情時的收合模式維持一行，改成截狀態句、回合號保留。`party/src/features/room/phone-layout.css`、`RoundBar.tsx`。

## 驗證
- 單元／整合：`room-state.test.ts` 新增一案（擲骰＋送出→第 2；收回改字→第 1；GM 旁白前後都第 1；只擲骰的回合算一輪；重生中與重生後；即時與快照一致），`room.test.tsx` 兩處字串改成每幕計數，`DesktopRoom.test.tsx` 新增交給手機後可收、配對後不顯示。
- 實測：測試通道桌面＋本機 Node 伺服器＋WebKit iPhone 13 兩支手機（Haiku 真跑）；截圖在 `<scratchpad>/hostbar/shots/`。

## 需要部署
- 手機端：`party/` 先 `npm run build` 再 `wrangler deploy`（手機客戶端跟 Workers 一起上，指令見 [multiplayer-friends-test](multiplayer-friends-test.md)）；伺服器邏輯與協定沒改。
- 桌面端：桌面版內嵌同一套 party 元件，第 1、2 項跟下一版桌面發佈。手機端與桌面版要同一批上，否則房主（舊桌面）看到流水號、手機看到每幕序號。

## 已知限制
- 連續兩次收回且第一次那回合有擲骰：故事串只留一行收回提示，第一次的擲果列會被當成一輪，回合數多 1（罕見，輕防）。

## 觀察（範圍外，未立案）
- party 的德文等語系缺回合列字串，回退成英文（實測德文介面看到 "Everyone's turn to act"）。

> 結案（2026-11-15）：已合 main；手機端待部署（見 tasks/room-fixes-deploy.md）。
