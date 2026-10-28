# multichar-next-speaker-silent — 實作方案（三方共識）

## 拍板

- 作者選 C〔作者裁決 2026-10-28〕：「預設維持一次旁白，旁白下方出現『讓 X 接話』鈕，另在送出鍵旁加『送出並讓點名角色接話』走 B（GM 旁白＋被點名角色接一句就停）」。
- 目標：想要角色接話的玩家一鍵就有，預設呼叫數不變（對 GM 一般送出＝1 次）。
- 方案經主線、Sol、獨立 Opus 審查三方共識（2026-10-28）；下文標〔主線裁決〕者是審查裁定的模型判斷，非作者裁決。

## 名詞

- **點名**：`gm_narrate` 回傳的 `next`，後端 `chat_assembly::next_speaker` 已解成在場角色 id／`PLAYER_SENTINEL`／null；名單外的名字已是 null，前端不比對名字。
- **C 鈕**：旁白下方「讓 X 接話」。**B 鈕**：送出鍵旁「送出並讓點名角色接話」。

## 結構拆檔（獨立 commit，先做）〔主線裁決〕

`chat/useChatController.ts` 的「逐字稿存取」搬到 `chat/useTranscriptStore.ts`：`events`／`eventsLive`／`setEvents`、本地改動追蹤 `writes`／`beginWrite`、`hydrate`、`reload`、`replaceEvent`、`appendEvent`。世代仍只有 controller 那一份（store 收 `generation` ref，世代變時換新的 `writes`）；`appendEvent` 既有副作用照舊（蓋 actionId、回錯記 `appendFailed`、落檔後清收回堆疊、世代不同不碰畫面）。跑回合的函式不搬。controller 降到 1000 行以下。

## 行為改動

### 1. 共用 `callAndReply(next, started, after?)`

`gmAdvance` 迴圈體抽出：`next === PLAYER_SENTINEL` → 寫「GM 請玩家發言」列就停；否則寫 `gm_call` 列 → 檢查 `stopRequested` → `replyOnce`。`gmAdvance`、B、C 共用（`gm_call` 列是角色模型知道自己被點的依據——AI 歷史裡旁白已剝掉點名行）。

**C 不疊列**：帶 `after`（那則旁白的 id）時，只看「該旁白之後到目前尾段」有沒有同名 `gm_call`，有就不再寫；不搜全逐字稿。

### 2. 正文落檔即記（`narrateOnce` 的 `onLanded`）

- `narrateOnce` 收一個選用回呼，GM 正文（含停止留下的半截）落檔成功當下呼叫 `onLanded({ id, next })`；停止或截斷的旁白 `next` 一律給 null（不得接力）。`gmAdvance` 不傳，行為不變。
- 呼叫端據此記 `narrated`：之後 `state_update` 落檔、`refreshState`、`markCliConnected`、`gm_call`、接話任何一步拋錯，**一律保留玩家句、失敗彈窗不帶 draft**（玩家句已不是檔尾，帶 draft 會誘導重送）。一般送出也套用。
- 同一刻設點名暫存 `pendingCall = { narrationId, characterId, gen }`，只限單次旁白入口（`submitTurn` 對 GM、`gmNarrate`）、`next` 是角色 id、世代相符。

### 3. 點名暫存（記憶體）

- 換桌／換幕（世代變）即失效，列已知限制〔主線裁決：不做 worldId＋scene 映射，罕見狀況輕防〕。
- 不選「後端把 next 寫進旁白事件」：要動 TranscriptEvent 契約（Rust＋TS）、代落路徑與匯出，只為重開 app 後保住一顆鈕〔模型判斷·未裁決〕。

### 4. C 鈕顯示（純函式 `next-speaker-call.ts`）

`callableAt(events, pending, activeIds)` 回傳要掛鈕的事件 index 或 null：pending 存在 ∧ 最後一則非 system 事件的 `id === narrationId` ∧ 角色在 `characters.active`。玩家再發言、別的回覆、收回旁白、角色封存或隱藏都自然失效；旁白後跟著的狀態更新、登場、點名列是 system，不使鈕失效。接話成功後不清 pending：玩家收回那句對白，鈕回來當重試入口。生成期間（busy）不畫，結束後重推導。

### 5. C 鈕位置與行為

- 渲染在那則 narration 事件**正下方**；文字「讓{name}接話」，次要小鈕；busy／唯讀／換幕容量滿時不出現或停用。
- 點下＝`callNominated()`：取得 busy 後先清 `stopRequested`，建立自己的 actionId、世代與失敗收尾，`callAndReply(characterId, started, narrationId)`；不重跑 GM、不改發言對象。寫點名期間按停止就不進 `replyOnce`。
- 經 App.tsx `lockedChat` 的 `whenTurnReady`。

### 6. B 鈕位置與行為

- `composer-actions` 右側、送出鍵左邊，不進左邊三顆相連的 `btn-seg`；短標「送出並接話」，title／aria-label 全文。
- 顯示：`gmTargeted && !gmOnly && !castEmpty && nominates`；`canStop` 時與送出一起被停止鈕取代；停用條件＝送出的停用條件＋輸入 trim 後為空〔主線裁決〕。Enter 永遠是一般送出。
- 行為：`sendRelay()` → `submitTurn(input, true, relay)`：取得 busy 後先清 `stopRequested`；落玩家句 → `narrateOnce` → 若 `onLanded` 給了點名、沒按停止、世代沒變 → `callAndReply` 一次就停。最多 2 次呼叫。點名到玩家：寫「GM 請玩家發言」列後停。
- 經 `lockedChat` 的 `whenTurnReady`。卡片介面送出（`submitText`）不給 B。

### 7. 不點名的桌隱藏 B

- 後端把 `interface_takeover_table` 換成 `gm_table_mode`，回 `{ gm_only, nominates }`；`nominates` 用 `data::requires_present_report`（即 `gm_turn_format(...) == Narration`，含 characters 例外），與 GM 指示同一判定。
- 前端 `useGmOnlyTable` 一併讀回，與 gmOnly 同時機載入／刷新（App.tsx 換桌、復原、重構套用後）、同一個讀取世代防護。
- 重讀時機：除了換桌、復原、重構套用後，主區名單或任一張卡的封存／auto_hidden 旗標一變也重讀（`useGmOnlyTable` 的名單鍵）（後端略過封存與 auto_hidden 旗標的卡；旗標只在換幕結算時變，劇情帶出場不改它，所以隱藏卡被帶出場當下結果不變，換幕後才變）〔主線裁決〕。
- 讀取失敗：同一桌保留上次讀成的結果；這桌從沒讀成就當一般桌、藏 B（不多花錢）；不另做重試，名單變動重讀就是自然重試〔主線裁決〕。
- C 不另擋：這種桌後端不要求點名，C 鈕通常不出現。GM 推進本案不動，另案。

### 8. 點名到玩家

一般送出：不出鈕、不寫列——畫面與 AI 歷史跟現在一樣。B：寫「GM 請玩家發言」列後停。

## i18n

新鍵 `sendRelay`（短標）、`sendRelayHint`（全文）、`callNominated`（「讓{name}接話」）、`callNominatedHint`；10 個語系檔全補。

## 已知限制

- 重開 app、換桌或換幕再回來，C 鈕不會復現（點名只存記憶體）。
- GM 推進在不點名的桌仍會退化成一次旁白（另案）。
- 「會不會點名」第一次查詢失敗、之後名單與旗標都沒變時，B 可能一直藏著（名單變動重讀不保證立即恢復；換桌再回來會重讀）。
- 同一張卡換卡檔、切換「允許卡內 regex」不觸發重讀；換桌、復原、重構套用、換幕後才會讀到新結果。
- 網頁版 `web/` 沒有這條遊玩流程，本案只動桌面版。

## 測試

hook 測試（照 `chat-send-failure.test.tsx` 的假 invoke）＋純函式單測＋PlayView 測試：

1. 一般送出對 GM：1 次 `gm_narrate`、0 次 `chat_with_character`、無 `gm_call`、C 鈕條件成立。
2. 點 C：`gm_call`＋1 次 `chat_with_character`，之後鈕消失；收回對白後鈕回來，再點 C 不疊 `gm_call`；連續重試點 C 不疊。
3. B：`gm_narrate`＋`gm_call`＋`chat_with_character` 一次就停。
4. 點名玩家：C 無鈕無列；B 有「請玩家發言」列、0 次接話。
5. 失效：玩家再發言、收回旁白、換桌、角色封存。
6. B 在旁白後停止：不接話；之後點 C 會接話（停止旗標已清）。
7. 正文落檔後各後處理（`state_update`、`refreshState`、`gm_call`、接話）失敗：玩家句留著、彈窗無 draft；一般送出同。
8. 空白輸入時 B 停用；不點名的桌 B 隱藏。App 層（走 AppWorkspace 傳給 PlayView 的 handler）：換桌鎖持有、桌型未讀回時 B／C 零玩家落檔、零點名列、零模型呼叫，解除後照常；名單變動後重讀；重讀失敗保留上次結果；從沒讀成藏 B。
9. 既有 `gmAdvance` 測試不改、全綠。

`npm run verify` 全綠。

## GUI 實測（test-harness）

`npm run harness:build`，`node scripts/harness.mjs` 帶 scratchpad 獨立 `--root`（全機單實例：動前 `ps` 確認，別人在用就等，不關別人的；不碰 `~/Documents/TableTavern`、`~/Library/Application Support/TableTavern`）；非介面雙角色桌（`TestCards/` 兩張一般角色卡），低階模型。每步截圖：

1. 對 GM 一般送出 → 旁白正下方有「讓 X 接話」、0 則角色對白。
2. 點鈕 → 「GM 請 X 發言」列＋X 一句，鈕消失。
3. B 鈕送出 → 一次得到旁白＋點名角色一句。
4. 點名到玩家時無鈕（遇到才拍，不刻意誘導）。
5. 收回旁白 → 鈕消失。

## 業界對照

- SillyTavern 群組：送出依 activation strategy 自動挑人回（Natural＝被提名者優先）；Manual 模式沒人自動回，成員列表每人一顆「speak」鈕（`group-chats.js` `action === 'speak'`）。C 鈕＝把那顆鈕放在 GM 點名當下、預選好人；B＝Natural 的「一句一回」。
- Marinara：送出依 response order 回話，輸入框旁快捷選單提供 Post only／Guide reply 等送出變體（`ChatInput.tsx` quickReplyActions）。B 鈕放送出鍵旁同這思路。
- 差異：兩者沒有 GM 調度者，誰回話由規則或 decision model 決定；我們由 GM 旁白順帶點名、不另開呼叫，預設不替玩家多花一次。
