# room-table-chat：桌邊聊天（方案）

來源：玩家回饋「需要房主與玩家之間、或玩家彼此之間的私下溝通視窗，常有人不小心發出和別人劇情衝突的內容，需要先溝通（倒數中或許要關？）」。作者定順序 1→2→3 的第 3 案〔作者裁決 2026-11-06〕，與案 1、案 2 各自分支並行〔作者 2026-11-06 要求〕。全篇取捨未標來源者為〔模型判斷·未裁決〕。

案別：中案偏大，核心估約 290 行（伺服器約 90、客戶端約 200）。碰信任邊界輸入（玩家文字進伺服器、顯示給其他玩家）：方案先審（本輪由主線審〔作者 2026-11-06 指示〕），做完再驗收一次。

## 0 設計結論

- 一個「聊天」分頁，大家都有；發送時選對象：**大家**，或**某位成員**（含房主）的私訊。公開與私訊同一條時間線，私訊標「（私訊）」。
- 聊天是桌邊閒聊（OOC）：**不進故事串、不進 GM 提示詞、不進匯出**。執行端永遠收不到（只送給成員連線）。
- **倒數中不關聊天**：倒數是房主的壓力工具，聊天是玩家避免撞行動的手段，關了只會把協調趕去 LINE。
- 伺服器保存最後 200 則（跟房間狀態一起落地），手機熄螢幕重連、重開頁面都看得到；房間閒置清除時一起消失。私訊因此會短暫存在伺服器上（跟草稿、被收回的原文同一等級）。

## 1 伺服器

- 型別（`types.ts`）：`ChatMsg { id: number; at: number; from: string; to: string | null; text: string }`；`RoomState` 加 `chat?: ChatMsg[]`、`chatNo?: number`（舊房間沒有這兩欄，讀取一律 `?? []`／`?? 0`）。
- 指令 `chat.send`，驗證表 `"to?:s text:s"`。處理（`member.ts`）：
  - 房間未關（lobby／playing／transition 都可；不要求入座，剛進房還沒選角也能聊）。
  - 文字：`cleanText(raw, 1, 500)`，不合回 `E_TEXT_INVALID`。
  - `to`：有給就必須是存在、未封鎖、不是自己的成員，否則 `E_STATE_CONFLICT`。
  - 限流：`rate(tx, \`c:${m.id}\`, 10, 10_000)`（每人 10 秒 10 則）。
  - 寫入：`id = ++chatNo`，推進 `chat`；超過 200 則、或 `new TextEncoder().encode(JSON.stringify(chat)).length` 超過 64 KiB（UTF-8 位元組，不是 UTF-16 長度），就從頭丟到符合為止（Workers 殼把整份狀態當一列存進 DO 的 SQLite，聊天不能把狀態撐大到寫不進去）。測試用中文訊息觸發裁切。
  - `to` 省略＝公開，伺服器存成 `null`；驗證表只接受省略或字串，不接受 `null`。
  - 房間已關回 `E_ROOM_CLOSED`。
  - 送達：只送成員連線，不用 `toAll`（`toAll` 含執行端）。公開 → 對每位未封鎖成員 `tx.toMember(id, "vol", { type: "chat", msg })`；私訊 → 只送 `from` 與 `to` 兩人。回 `ok { result: { id } }`。
  - 不發持久事件（不進事件日誌、不佔 seq）。
- 快照（`snapshot.ts`）：`Snapshot` 加 `chat: ChatMsg[]`＝`visibleChat(s, viewer)`（`to === null || from === viewer || to === viewer`）；viewer 不是成員（執行端的 welcome 也走 `buildSnapshot`）一律回空陣列，公開訊息也不給執行端。`liveValues` 也加同一份 `chat`（補事件路徑的重連要能補回離線期間的訊息）。
- 協定文件 `.ai/plans/multiplayer-room-protocol.md`：結案時由主線補一節實作差異（本案與 room-round-timer 一起），Grok 不動它。
- 匯出：`toWebSave` 從事件建，`chat` 不在事件裡，自然不含；加一條測試鎖住。
- 被踢／封鎖者：既有訊息留著（別人已看過），之後送不進（成員已不在或 `to` 驗不過）。
- 測試（`core/chat.test.ts` 新檔）：公開訊息全員收到、執行端收不到；私訊只兩人收到；快照與 live 依收件人過濾；200 則環；限流；`to` 給自己或不存在者被拒；關房後被拒；匯出不含聊天；舊狀態沒有 `chat` 欄位時照常。

## 2 客戶端

**玩家看到什麼**：分頁列多一個「聊天」。人在別頁時，有新訊息分頁上出現未讀數字；故事頁回合列下方多一條一行的「阿傑：我先去找酒保，你別撞」（截到一行），點了切到聊天頁。聊天頁是一般聊天室：訊息由舊到新、自動捲到最新，每則寫「阿傑 → 大家」「房主 → 你（私訊）」「你 → 阿凱（私訊）」與時間；底部一列：對象下拉（大家／各成員，房主標「房主」）＋輸入框（一行起跳、隨內容長高，沿用 `.action-row`）＋「送出」。

- `room-state.ts`：`RoomView` 加 `chat: ChatMsg[]`。快照與 `welcome.live` 整份換；vol `chat` 依 `id` 去重後追加（保持升冪），客戶端同樣只留最後 200 則（長時間在線不累積；未讀數只算還留著的）。`members` 裡查不到的 `from`（已被踢）顯示「已離開」。
- `RoomScreen.tsx`：tab 加 `"chat"`（順序：故事、聊天、名單、房主、GM 額度）。未讀：`seenChatId` state，這次掛載收到第一份 view 時初始化成當時最後一則的 id（重新整理不會把舊訊息全算成未讀）；之後重連換快照不重設（手機熄螢幕期間到的訊息保持未讀）；只有「聊天頁開著**而且**頁面可見（`document.visibilityState === "visible"`）」時才更新成最後一則 id：切到聊天頁、聊天頁開著收到新訊息、以及從背景恢復可見且停在聊天頁，三個時點各做一次。停在聊天頁熄螢幕期間收到的 vol 或重連快照不推進。未讀數＝`chat.filter((m) => m.id > seenChatId && m.from !== me).length`，顯示在分頁鈕上（`.tab-badge`）。未讀計算與 `ChatNotice` 一起放 `features/chat/`（例如 `useUnread.ts`、`ChatNotice.tsx`），`RoomScreen` 只留分頁與組裝。故事頁的一行提示：未讀 > 0 且不在聊天頁時，`DelegatedBanner` 之後渲染 `ChatNotice`（最後一則未讀的「名字：內文」，`button.banner` 樣式，點了 `setTab("chat")`）。手機 `.reading` 時隱藏提示列。測試補三種：重新整理後舊訊息不算未讀；故事頁斷線重連期間到的訊息算未讀；聊天頁開著但頁面不可見時收到訊息不歸零、恢復可見才歸零。
- 新檔 `party/src/features/chat/ChatPanel.tsx`（新功能、自成資料夾，照 docs/STRUCTURE.md）：列表、對象下拉、輸入列、送出（`act("chat.send", { text, to })`，成功清空輸入框，失敗紅字在列下）。對象預設「大家」；送過私訊後維持那個人，直到自己改。桌面嵌入（`c.pairForm === false`）Enter 送出、Shift+Enter 換行，輸入法組字中（`e.nativeEvent.isComposing` 或 `keyCode === 229`）的 Enter 一律不攔，交給輸入法選字；手機 Enter 是換行、只用送出鈕（軟鍵盤沒有 Shift）。訊息內文一律 React 文字節點。字數上限 500，超過時送出鈕停用並寫剩餘。
- 樣式：`App.css` 加 `.chat-list`（flex 1、捲動）、`.chat-msg`（名字 13px 系統字、內文 pre-wrap、私訊用 accent 淡底）、`.chat-compose`（sticky 底部，同 `.action-box` 的底色與 safe-area）、`.tab-badge`（小圓數字）。不新增版面高度給故事頁（提示列只在有未讀時出現）。
- 不做：訊息刪除、已讀、表情、@提及、房主靜音；要的話另立案。
- 測試（`room.test.tsx`）：A 送公開訊息 B 看得到且分頁有未讀 1、切過去歸零；A 私訊 B，第三人 C 看不到；故事頁提示列點了切頁；超過 500 字送出鈕停用。

## 3 文案

鍵加進 `party/src/i18n/zh-TW.ts`（正典）、`zh-CN`、`en`；其餘七語填英文。不計入核心。

## 4 驗證

`cd party && npm run typecheck && npm test && npm run format`；根目錄 `node scripts/check-i18n.mjs`、`node scripts/check-structure.mjs`。完整 `npm run verify` 由主線合併後跑。手機截圖（375×812）：聊天頁（公開＋私訊各一則）、故事頁的未讀提示列與分頁數字、對象下拉打開。

## 4a 施工結果與已知限制

- 實作核心約 340 行（程式約 260、樣式約 85）、測試約 360；Grok 施工，主線補五處（聊天面板常駐、送出只清該次內容並防重複提交、私訊佔位字、提示列私訊標記、空狀態）。Sol 方案審 2 輪（必改 3 全採）、驗收第 1 輪必改 2（切頁重設對象、送出清掉新打的字，已修）。
- 已知限制：只聊天不發事件，所以不刷新房間閒置時間；`to: null` 走既有 `E_BAD_FRAME`（客戶端不會送）；訊息只顯示時分，不標日期與回合；提示列只顯示最後一則未讀。
- 截圖觀察（主線親看）：私訊淡底與公開訊息區分清楚；對象下拉是原生 select，手機上展開樣式由系統決定。

## 5 部署注意

協定多了 `chat.send` 與快照／live 的 `chat` 欄位：舊伺服器回 `E_BAD_FRAME`，客戶端照一般錯誤顯示；新伺服器對舊客戶端多給的欄位無害。合進 main 後先 `wrangler deploy` 再打包（等作者）。

## 6 與並行案的重疊

案 1 動 `RoomScreen` 故事分頁內容與 `RoundBar`，案 2 動 `RoundBar` 動作列與 `ActionBox`；本案動 `RoomScreen` 的分頁清單與 `DelegatedBanner` 後面一行、新資料夾 `features/chat/`、`room-state`、伺服器。衝突預期在 `RoomScreen` 的 import／tab 區、`room-state` 的 `RoomView` 欄位、i18n 字典檔，由主線解。
