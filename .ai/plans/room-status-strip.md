# room-status-strip：桌邊狀態列＋房主提醒（方案）

來源：玩家回饋四項（房主提醒、倒數開關、私下溝通、使用者狀態）的第 1 案；作者定順序 1→2→3〔作者裁決 2026-11-06〕，回饋來自桌面版開房的房主。三案共用一個結論：連線、每人行動狀態、倒數都已在協定裡，缺的是放在哪與主動提醒。全篇取捨未標來源者為〔模型判斷·未裁決〕。

案別：中案，核心估約 150 行（純客戶端，不改伺服器與協定）。做完由主線驗收一次（本輪審查由主線擔任〔作者 2026-11-06 指示〕）。

依據：`party/src/features/room/{RoomScreen,RoundBar,StoryView,NudgeButton,DelegateButton,room-state}.tsx|ts`、`controls/HostPanel.tsx`、`App.css`、`phone-layout.css`；現況截圖（scratchpad/shots）主線親看：房主頂端擠（倒數、結算鈕、還在等＋催一下三層）、玩家摘要收合時看不到誰還沒動、大廳故事頁整片空白。

## 1 桌邊狀態列（SeatStrip）

**玩家看到什麼**：故事頁回合列正下方一列座位 chip（頭像＋角色名＋狀態），手機與桌面嵌入同一份。誰在打字、誰送了、誰離線、誰交給 AI，一眼看完；大廳時看得到誰進房還沒選角。

- 位置：`RoomScreen` 故事分頁，`DelegatedBanner` 之下、場景列之上。新檔 `party/src/features/room/SeatStrip.tsx`。房間 `closed` 不顯示。
- 列誰、順序：
  - playing／transition：入座成員；有 `round.roster` 時照名單順序，其餘入座者（代管中、下回合才登場）接在後面。未入座的旁觀者不列（進房用 §2 的提醒）。
  - lobby：入座者在前（顯示角色名），未入座者在後（顯示暱稱，標「還沒選角」，淡）。
- chip 內容：頭像 22px 圓（`PublicCard.avatar` 有圖用圖，沒圖用角色名首字、邊框用卡色）＋名字（角色名；`title` 放「暱稱 · 角色名」）＋狀態徽章。
- 狀態徽章（由上往下第一個成立者）：
  1. 沒連線：整顆淡（opacity .5），徽章「離線」。
  2. `delegated`：徽章「AI 代管」。
  3. 回合 `acting` 且在名單：`drafting`→「打字中」＋動態三點（CSS 動畫，`prefers-reduced-motion` 時靜態）；`submitted`→「✓ 已送出」；`skipped`→「交給 GM」；`pending`→不出徽章。
  4. 回合 `acting`、已入座但不在名單（下回合才登場）：淡，徽章「下回合」。
  5. 非 acting（生成中、卡住、房主回合）：`round.carry` 裡 submitted／skipped 照第 3 條顯示，其餘不出徽章。
- 房主點別人的 chip（chip 是 button）：`PartyMenu` 小選單，項目依狀態出現：
  - 「催一下」：acting、pending／drafting、在線時出現；按過或被擋（`E_RATE_LIMITED`）後 30 秒內改成「已提醒」停用。冷卻狀態要跨選單開關保留（每位一份 until），邏輯從 `NudgeButton` 搬進來，`NudgeButton.tsx` 刪掉。選單項目保留 `data-testid="nudge-<memberId>"`。
  - 「交給 AI 代管」／「取消代管」：`canDelegate(view)` 時出現。把 `DelegateButton` 的送出與「已送出要先確認」邏輯抽成 `useDelegate(view, act)`（回 `start(memberIds, on)`、確認框節點、error），`DelegateButton` 與狀態列共用，行為不變。
  - 非房主、或點自己：chip 是 span，不可點。
- 回合列同步刪掉：「還在等：…」整段（`waiting`／`waitingShown`／`nudgeable`、名字旁的催促與代管鈕）、對應文案鍵 `roundWaiting`。結算鈕的「（N 人還沒送）」照留（就地算名單）。`expandable` 只看 GM 模型與倒數。`reopen-note`（重開後誰還沒連上）照留。
- 版面：`.app .seat-strip` 一列、不換行、超出橫向捲（隱藏捲軸），高 36px（min-height），底線、surface 底；chip 圓角藥丸、surface-2 底、13px 系統字，徽章 12px。手機：`.reading` 時與輸入框聚焦時都隱藏（加進 `phone-layout.css` 既有那兩組選擇器）。桌面嵌入不另寫 CSS，除非 `src/styles/room.css` 需要一行對齊。

## 2 房主提醒（toast＋提示音）

**房主看到什麼**：故事區頂端浮出一條 4 秒的小提示（最多疊 3 條，點了就收），同時一聲短提示音、手機震一下。四種事：有人進房、有人選了角色、有人送出或交給 GM（附還剩幾人）、行動中有人斷線。只有房主收到；手機房主與桌面房主一樣。

- 新檔 `party/src/features/room/host-alerts.ts`（`useHostAlerts(view, { sound })` 回 toasts 與 dismiss）＋ `Toasts` 元件（可與 hook 同檔或拆 `Toasts.tsx`，照 docs/STRUCTURE.md）。只在 `view.room.hostId === view.me.memberId` 且房間未關時推導；用 ref 記上一份 view，每次更新比對：
  - `members` 多了別人 →「{暱稱} 進房了」；大廳時「{暱稱} 進房了，還沒選角」。
  - 別人的 `cardId` 從 null 變有 →「{暱稱} 選了 {角色名}」。
  - 同一回合、acting、別人的名單狀態變 submitted →「{角色名} 已送出，還剩 {n} 人」；變 skipped →「{角色名} 交給 GM，還剩 {n} 人」；n 為 0 時改句「{角色名} 已送出，全員到齊」（回合會自動結算，GM 主機離線時回合列已寫原因，不另提醒）。
  - 同一回合、acting、別人在名單裡還沒送、`connected` 變 false →「{角色名} 斷線了」。
  - 上一份 view 是 null（第一份快照）不出提示；重連重抓快照後的差異照出（等於「你不在時發生的事」）。回合號變了就不比名單狀態。
- Toast 容器放在狀態列之後、場景列之前：`position: relative; height: 0`，子項絕對定位浮在故事區頂端，不佔版面。`role="status"`。
- 提示音：WebAudio 合成（660 Hz 80 ms → 880 Hz 80 ms，gain 0.15），`AudioContext` 第一次用才建立，`state !== "running"` 先 `resume()`；1 秒內多條只響一次；任何失敗靜默。震動 `navigator.vibrate?.(80)`。全部包 try/catch。
- 開關：房主頁加一個 checkbox「有人進房、送出時提示音」（`hostAlertSound`），預設開；存 `globalThis.localStorage` 鍵 `tt-party:host-sound`（try/catch，同 i18n 的存法）。toast 本身不可關。

## 3 文案

鍵加進 `party/src/i18n/zh-TW.ts`（正典）、`zh-CN`、`en` 三份翻譯；其餘七語照現行慣例填英文（`scripts/check-i18n.mjs` 會擋缺鍵與寬度）。不計入核心。

## 4 測試（`party/src/features/room/room.test.tsx`，總行數不超過核心兩倍）

- 狀態列：打字中→已送出→交給 GM 的徽章變化；離線淡化；大廳列出未選角者。
- 房主選單：催一下送 `host.nudge`，之後顯示「已提醒」；非房主 chip 不可點。
- 提醒：別人送出時房主看到「已送出，還剩 1 人」；非房主沒有 toast。
- 既有引用 `round-waiting`、`nudge-`、`waiting-delegate-` 的測試改到新位置。

## 5 驗證

`cd party && npm run typecheck && npm test && npm run format`，最後根目錄 `npm run verify` 全綠。手機截圖（375×812）：玩家故事頁（三種徽章）、房主故事頁（點 chip 開選單）、大廳房主頁、房主收到 toast；桌面嵌入版面由主線另看。

## 5a 施工結果與已知限制

- 實作核心淨增約 520 行（TS 約 375、CSS 約 145），遠超立案估的 150：狀態列的徽章、頭像、選單、冷卻與 toast 推導各自成段，CSS 含動畫與 toast 浮層；測試約 75 行。Grok 施工；主線補四處（提示音在 click 裡解鎖並接住 resume 拒絕、toast 改 flex 直排不重疊、刪無用 CSS、狀態列改換行）。Sol 驗收 2 輪：必改 2＋1（提示音手勢、toast 疊放、觸控要用 click），全修。主線看截圖後裁定：手機橫向捲動會把沒送的人藏到畫面外，改成放不下就換行（6 人兩行，多佔約 36px）、名字超過 5 字截斷、桌面 toast 限寬 28rem；桌面嵌入截圖是近似版（真 DesktopRoom 接假核心、左欄留白），狀態列在回合列下方位置合理，不改 CastRail。
- 已知限制：關掉提示音仍會震一下；「你不在時」進房又選角的人會同時報兩條；還剩 0 人時送出與交給 GM 都寫「已送出，全員到齊」；大廳的 chip 沒有可做的事所以不可點；iOS Safari 不支援震動。

## 6 觀察（範圍外，不立案）

- 房主頁「邀請新朋友／換新連結／匯出紀錄」三顆鈕換行排列，看不出輕重。
- 故事與回合列用角色名、名單頁用「暱稱 · 角色名」，同一人兩種叫法；狀態列用角色名、title 補暱稱。
- 大廳故事頁只有「等房主開始」與開始鈕，整片空白；狀態列補上後至少看得到誰到了。

## 7 後續案（同一批回饋，作者已定順序）

- 案 2 `room-round-timer`：房主回合列「⏱ 倒數 ▾」（1／2／5 分鐘當回合開始、倒數中「停止」），伺服器加一個指令只改 `deadline`／`pausedRemainingMs` 並發既有 `round.deadline`；房主頁設定改名「每回合預設倒數」；玩家行動格上方「房主開了倒數」橫條。小案，核心約 60。
- 案 3 `room-table-chat`：「聊天」分頁，對象選大家或某位成員（含房主）；不進故事、GM 提示詞、匯出；房間狀態留最後 200 則，快照與重連 live 依收件人過濾，即時走 vol；分頁未讀紅點、故事頁一行 toast。倒數中不關聊天。中案偏大，核心約 260；信任邊界輸入，方案送審。
