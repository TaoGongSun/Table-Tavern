# interface-table-state-report — 未重構的真介面桌不保證回報 time／place／present

## Summary
卡片自帶真介面、還沒重構的桌（WestFantsy UI 版、bcd368、DongeonMaster 等），GM 回合走中性版指示（內容是〔作者裁決 2026-10-04〕），不要求 ```state 圍欄，time／place／present 不保證有值；只有 `response.rs:344` 一帶從卡片自己的狀態格式碰巧抽得到時才有。登場與在場裁切因此可能失去依據。由 gm-no-state-fence-worldbook-table 審查提出，Sol、Grok 都建議另立〔模型判斷·未裁決〕。

## 已知線索
- 不能直接在中性版加圍欄：卡片格式條目在時，附圍欄會和「不要輸出格式以外的東西」對打，卡介面可能掉（Grok）。
- 候選方向：從卡片自己的狀態欄讀 time／place／present，或另附圍欄；先量測實際缺欄頻率與裁切後果再提給作者。
- 動到中性版要作者裁決。

## Next action
2026-10-29 輕防重審：縮小成只量測〔模型判斷·未裁決〕。最小版＝挑 2–3 張未重構的真介面桌多輪續聊，量 time／place／present 缺欄頻率與在場裁切後果，結果交作者；不改中性版、不做「從卡片狀態欄讀值」。
刪掉的部分成已知限制：未重構的真介面桌不保證有 time／place／present（記在 [gm-no-state-fence-worldbook-table](../plans/gm-no-state-fence-worldbook-table.md)）。介面桌依〔作者裁決 2026-10-27〕不點名，main 已有「確定不點名的桌」判定，後果比立案時輕。
量出來缺欄多、要動中性版，仍需作者裁決。

## 驗收方向
未重構的真介面桌多輪續聊，time／place／present 有值，卡片介面不受影響。
