# room-small-fixes：房間三小事（方案）

來源：BACKLOG 三行〔模型判斷·未裁決；常識審查員〕，作者「先修房間小案」。

## 1 選角簡介剝 Markdown
`SeatPicker` 直接印 `blurb`（桌面 `public_md` 原文、單行 ellipsis）。改由客戶端 `seat-blurb.ts` 剝成純文字：有內文略過標題（含 Setext 標題），`{{char}}` 換卡名，`<style>`、`<script>`、註解連內容去掉。party 沒有 marked，不為這格加依賴，用正規式。不改 `seed.rs`：房主桌面沒更新也會好。桌面與手機共用同一元件。

## 2 手機顯示觀望
`storyOf` 只替 `act` 與 `skipped` 推段落，`timeout`／`host_resolved` 丟掉。補一段 `watched: true`，文字用桌面同一份 `transcript-marker.json` 的 `player_watch`，十語免新增。改在 `storyOf` 才不會重新整理就消失。

`room_lines`（`sync.rs`）比對鍵排除 `watched`、`handed`，留位置；這兩種是房間自己投影的空段，桌上 seed 不輸出，納入比對會在重開時誤判分歧、裁掉後文重送（`handed` 原本就有此問題，一併修掉；測試復現過）。

執行端的快照濾掉觀望行（只濾顯示投影，不重編段號，`keepThrough` 仍指伺服器原座標），手機保留，所以舊版桌面接新伺服器重開也不會誤判分歧。

要先部署伺服器。已知限制：
- 舊版桌面內嵌的房間畫面（手機同一份元件的舊版）收到快照時，觀望段顯示成空白的角色段，更新 app 後消失。
- 重開補送時，故事尾端只剩觀望行可能被裁掉；不會錯裁真劇情。

## 3 換裝置接回座位：玩家認領＋房主確認

身分只認裝置雜湊；暱稱不是憑證，只當佐證。接手一律要房主確認（Sol、Fable 方案審結論，主線定案）。不做「自動刪沒座位的舊列」與 `member.leave` 刪列。

### 玩家
- 選角頁：佔位者離線、且不是房主或出額度者的座位，顯示「已有人選・離線」＋「這是我的？」；暱稱相同加「對方也叫小華」小字。桌面嵌入版不顯示這顆。
- 按下後顯示「等房主確認；也可以先選別的角色」，其他座位照常可選。房主按「不是」時顯示「房主說這不是你的座位。」，連結變「再問一次」。
- 舊裝置回來：是沒座位的新成員，選角頁一行「房主把『格羅』交給了另一台裝置。不是你換的裝置？在聊天跟房主說一聲。」

### 房主
- 名單分頁頂端常駐卡片（一筆一張），沒有彈窗、toast、音效：「新加入的『小華』說『格羅』是他的，要交給他嗎？」，「剛加入的『小華』：在線」「原本選格羅的『小華』：離線 12 分鐘」，暱稱相同時附一行。按鈕「是，交給他」「不是」。
- 名單分頁鈕掛筆數（重用 `tab-badge`）；認領當下用既有桌邊提示出一條「小華想接回格羅，到名單確認」，不搶焦點。

### 協定（都不進事件日誌，其他玩家看不到）
- `seat.request { cardId }`（成員）：手上沒有認領且通過有效條件；限頻 10 分鐘 3 次（`sr:<memberId>`）。不符回既有錯誤碼（`E_STATE_CONFLICT`、`E_RATE_LIMITED`）。
- 有效條件 `seatRequestValid`（送出、確認、掃描共用）：status 為 lobby／playing、沒被封鎖、自己沒座位、沒有建卡／匯入工作、沒有卡檔上傳（含尚未派工的 `Live.uploads`）；座位有佔位者；佔位者不是自己、依 `isOnline` 離線、不是房主或出額度者，且仍是認領當時那位。
- `seat.request.resolve { memberId, requestId, accept }`（房主）：`requestId` 原樣回傳，不符回 `E_STATE_CONFLICT`，延遲的確認批不到新認領。`accept` 另驗有效條件，同一筆交易移座＋移除舊列，只發既有的 `seat.reassigned`、`member.kicked`；拒絕只驗版本與房主權限，只清認領。
- `vol seat_request { cardId|null, refused? }`：只給認領人。
- `vol seat_requests { list: [{ requestId, memberId, cardId, occupantId, offlineSince }] }`：只給房主；房主換人時給新房主完整清單、給舊房主空清單。
- `vol seat_handed { name }`：只給舊裝置，一次性。
- 快照加 `me.seatRequest`（cardId 或 null）與 `seatRequests`（只房主）；`welcome` 的補事件路徑另送上述 vol（同 `called`、`returned` 的做法）。

### 狀態
- `Member.seatRequest?: { requestId, cardId, occupantId }`：存在認領人身上，一人一筆。
- `Member.offlineSince?: number`：只當顯示用，最後一條連線斷時記，`hello` 時清；休眠醒來補的斷線（`stale`）不記，房主看到「離線，時間不詳」。
- `RoomState.handedOver?: Record<dev雜湊, 卡名>`：確認交接時寫，舊裝置以新成員 hello 時送 `seat_handed` 並刪除。新成員一律給完整快照，不沿用舊身分的 `lastSeq`。

### 失效
`reduce` 在每筆輸入後（單一掛點 `settleSeatRequests`）用有效條件掃一次，不成立就清掉並通知認領人與房主；舊裝置回來、開始建卡或上傳、換幕都因此失效。`member.leave` 明寫清掉（列還在，掃不出來）；被移除的人連列一起消失。

### 私密
`seatRequest`、`handedOver` 不展開給公開名單或執行端：公開名單逐欄挑選，房主清單由 `seatRequestsFor` 逐欄組出。

### 重用
`member.reassign`、`member.kick` 的本體抽成 `moveSeat`、`dropMember`，三個入口共用，舊指令行為不變。代管、草稿、回合名單、執行端座位表都走既有路徑。

### 已知限制
- 舊裝置回來時是沒座位的新成員，只看到一行提示；要交回由房主用既有 `member.reassign`。入口在房主分頁「到場整理」的「還沒入座」列（`member.reassign`）。`seat_handed` 是一次性訊息，送出後斷線可能漏看；`handedOver` 只留最近 8 筆，更早交接的舊裝置回來看不到提示。
- 換幕期間（status 為 transition）認領會失效，玩家要再按一次。
- 沒座位的舊列仍留著，房主用既有移除入口處理。
- 滿 12 人時新裝置進不來，房主要先移除舊列。
- 房主、出額度者換裝置不走這條（配對碼）。
- 舊列的私訊不跟著轉移：交接後新裝置看不到，舊裝置也回不去那個身分。
- 伺服器要先部署：舊伺服器不認 `seat.request`，會當壞訊框斷線；舊客戶端收到新 vol 只是忽略。

### 測試
伺服器：每條驗證各一、原子性（`resolve` 失敗狀態不動）、失效各路徑、私密性（其他成員收不到任何認領訊框）、房主換人、舊裝置提示。客戶端：選角頁鈕與等待行、房主卡片確認與拒絕。截圖：手機選角頁（離線座位、等待中）、房主名單卡片、舊裝置提示。

### QQ／微信
現況不是跳轉：`App.tsx:60` 只顯示引導橫幅，加入表單照常在下面。先照做不會有重複列；先加入、之後才換瀏覽器就會。連結帶 deviceId 不採用（微信轉發就交出身分），一次性權杖不採用（時間點控制不了）。

## 觀察
- 桌面 `StorySpec.tsx:38`、`RefactorResultDialog.tsx:223` 也直接印 `public_md`，是作者自己看的編輯情境，不是共用元件，沒動。
- `{{user}}` 留原樣：預覽沒有對應的玩家名。
- 預覽截圖「雷恩 是獅獸人鐵匠」的空格是原文（`{{char}} 是`），不是剝除殘留。
- 內建瀏覽器預設收起加入表單：這輪不做，等朋友實機數據。
- 引導文案寫「可能连不上」，原計畫是「WebSocket 多半正常」，等實機再定要不要改。
