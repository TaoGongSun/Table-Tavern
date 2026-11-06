# room-round-timer：回合倒數由房主當場開關（方案）

來源：玩家回饋「建議加倒數計時，時間到自動進下一回合，最好房主能啟動和關閉（有些玩家會拖很久）」。作者定的三案順序 1→2→3 的第 2 案〔作者裁決 2026-11-06〕；與案 1（room-status-strip）、案 3（room-table-chat）各自分支並行施工〔作者 2026-11-06 要求〕。全篇取捨未標來源者為〔模型判斷·未裁決〕。

案別：小案，核心估約 80 行（伺服器約 30、客戶端約 50）。做完由主線驗收一次。

## 0 現況已有的（不重做）

- 房主頁「行動倒數」是房間設定 `actionTimeoutSec`（不倒數／60／120／300／900）：每個 play 回合開出來就倒數（`round.ts` `openRound`）。
- 到期 `onAlarm` → `resolve(tx, "timeout")`：沒送的算觀望（`watchReason: "timeout"`）、草稿保留；GM 主機離線或收回等待中倒數暫停（`settle` 把 `deadline` 換成 `pausedRemainingMs`），回來續跑。
- 客戶端已處理 `round.deadline` 事件；回合摘要顯示「剩 1:20」／「倒數暫停」；最後 30 秒行動格自動展開（`urgent`）。

缺的是：不能「這回合有人拖了才開」、不能中途停；而且設定藏在房主頁，玩家不知道有這功能。

## 1 伺服器：`round.timer`

- 新成員指令 `round.timer`，驗證表 `"round:i action:s sec?:i"`。房主限定（`host(tx, m)`）、`roundAt(tx, p.round)`（當前 acting 回合，收回等待中會被擋）。
- `action: "start"`：`sec` 必須是 60／120／300 之一（其他回 `E_SETTING_INVALID`）；名單空（全員代管）回 `E_STATE_CONFLICT`。寫入 `pausedRemainingMs = sec*1000`、`deadline = null`，發 `round.deadline { round, deadline: null, pausedRemainingMs }`；交易尾端既有的 `settle` 會在「名單非空且執行端在線且沒有收回等待」時換成 `deadline` 並再發一次 `round.deadline`（沿用現有機制，不另寫分支）。已在倒數中再 start＝重設成新的秒數。
- `action: "stop"`：兩欄都清成 null，發 `round.deadline { round, deadline: null, pausedRemainingMs: null }`。只停這回合；下一回合照房間設定。
- 鬧鐘：`nextAlarm` 已把 `deadline` 算進去，不改。匯出、執行端、派工都不碰。
- 測試（`round.test.ts` 或 `host-actions.test.ts`）：start 後到期自動結算且沒送的是 timeout 觀望；執行端離線時 start 停在 pausedRemainingMs、上線後續跑；stop 後到期不結算；非房主 `E_FORBIDDEN`；錯誤秒數 `E_SETTING_INVALID`；名單空 `E_STATE_CONFLICT`。

## 2 客戶端

**房主看到什麼**：回合列的動作列（「結算這回合」旁）多一顆「⏱ 倒數 ▾」，點開選 1／2／5 分鐘，這回合立刻開始倒數；倒數中（含暫停中）那顆變成「停止倒數」。房主頁的設定改名「每回合預設倒數」。

**玩家看到什麼**：房主當場開倒數時，行動格上方出現一條「房主開了倒數：{n} 分鐘內送出」（沿用 `.nudge-bar` 樣式），手機震一下（`navigator.vibrate?.(200)`，不支援就只有橫條）。送出、交給 GM、回合結束、房主停止倒數時消失。回合一開就有倒數（房間設定）的情況不出這條，摘要的「剩 m:ss」已經夠。到期行為不變。

- `RoundBar`：`host && r.phase === "acting" && r.roster.length > 0` 時，`r.deadline === null && r.pausedRemainingMs === null` → `PartyMenu`（trigger「⏱ 倒數 ▾」，項目 1／2／5 分鐘，`onSelect` 送 `round.timer { round, action: "start", sec }`）；否則一顆 ghost 鈕「停止倒數」送 `action: "stop"`。放在 `.round-actions` 裡，結算鈕之後。
- `room-state.ts`：`RoomView` 加 `timerStarted: number | null`（客戶端記憶體，快照不帶）。`round.deadline` 事件：上一份兩欄皆 null、新值任一非 null → `timerStarted = e.round`；新值兩欄皆 null → `timerStarted = null`。`round.opened` 清成 null。
- `ActionBox`：`timerStarted === r.round && openTurn` 時顯示橫條（文案帶分鐘數：從 `deadline - now` 或 `pausedRemainingMs` 取整數分鐘，四捨五入）；震動用與催促同一套「每回合一次」的 `fired` 記錄。
- `HostPanel`：`hostTimeout` 文案改「每回合預設倒數」（三語＋英文）。
- 測試（`room.test.tsx`）：房主開 2 分鐘 → 玩家看到橫條與「剩 2:00」；停止 → 橫條消失、摘要回「輪到大家行動」；非房主沒有倒數鈕。

## 3 文案

鍵加進 `party/src/i18n/zh-TW.ts`（正典）、`zh-CN`、`en`；其餘七語填英文。不計入核心。

## 4 驗證

`cd party && npm run typecheck && npm test && npm run format`；根目錄 `node scripts/check-i18n.mjs`、`node scripts/check-structure.mjs`。完整 `npm run verify` 由主線合併後跑。手機截圖（375×812）：房主回合列的倒數選單、倒數中的「停止倒數」、玩家的橫條。

## 5 部署注意

伺服器多了一個指令：合進 main 後要先手動 `wrangler deploy` 再打包 app（等作者說）；舊版伺服器收到 `round.timer` 會回 `E_BAD_FRAME`，客戶端照一般錯誤顯示在鈕旁。

## 6 施工結果與已知限制

- 實作核心淨增約 90 行、測試約 150 行；Grok 施工，主線補兩處（收起後開倒數展開一次、房主自己不顯示橫條）。Sol 驗收第 1 輪必改 1（收起後橫條不出現，已修）。
- 已知限制：重新整理或重連後橫條不會回來（`timerStarted` 只在記憶體，摘要的剩餘時間還在）；預設倒數＋執行端離線時從 held 按繼續，`round.opened` 不帶暫停剩餘，上線續跑會被誤判成房主當場開而多出一次橫條（只影響提示來源，不影響倒數與資料，輕防）；`action` 非 start／stop 回 `E_BAD_FRAME`（核心既有慣例，客戶端不會送）。
- 截圖觀察：倒數選單打開時第一項預設高亮是 MoreMenu 的鍵盤焦點慣例，不改。

## 7 與並行案的重疊

案 1 刪掉回合列的「還在等」段、案 3 加分頁與文案；本案只動 `.round-actions` 內容、`room-state` 的事件處理、`ActionBox` 橫條、i18n 鍵。合併衝突預期只在 i18n 字典檔與 `RoundBar` 的 import 區，由主線解。
