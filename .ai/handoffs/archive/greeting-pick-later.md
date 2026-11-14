# greeting-pick-later — 跳過開場白後還能回頭選

匯入卡時按「先不要」跳過開場白後，沒有地方能再選。還沒開始玩的桌在空的對話區補一個「選擇開場白」入口，讀桌目錄留下的匯入原檔（收據上的 `import-source-*`）重開同一個開場白面板，貼出時照樣帶原檔識別與序號（巨集求值、重新重構取初始值都跟匯入當下一樣）。

- 案別：小案（立案估核心 70 行；實作新增核心 111 行、刪 10 行（估其中約 25 行是既有函式抽共用時的搬動與重排））。
- 範圍：一般聊天桌；VN 桌、重播桌、已遊玩（換過幕、第 0 幕有開場白以外的事件、回合在途）的桌不露出。角色卡與世界書兩條匯入路都適用。房間桌的畫面是 `DesktopRoom`（party 的 `RoomBody`），不畫 `PlayView`，本案不加入口〔模型判斷·未裁決〕；房間裡跳過開場白是否有同樣缺口未查明。

## 現況（已合 main 2026-11-14、Sol 驗收 1 輪無必改、截圖主線親看；結案）
- 後端 `world_scan::opening::later_openings`＋指令 `table_openings`；前端 `useLaterOpening`（露不露）、`ImportController.offerTableOpenings`（重開面板）、`PlayView` 的 `onPickOpening`（幕書籤下方 `story-actions`）。
- 有介面的卡（例：MVU 卡）匯完自動打開卡片介面整面蓋住對話區，入口要按 × 關掉介面才看得到〔模型判斷·未裁決：本案不改〕。
- 換開場白：「收回上一句」把開場白收掉後入口會回來，再選一則即可（收據序號會覆寫）。
- verify 17 步通過；測試通道實測截圖在 `<scratchpad>/four/greeting/shots/`。

## 下一步
無。
