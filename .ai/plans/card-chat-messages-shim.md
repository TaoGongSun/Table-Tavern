# card-chat-messages-shim — 計畫

交接：[handoffs/archive/card-chat-messages-shim.md](../handoffs/archive/card-chat-messages-shim.md)

## 定案範圍

- 卡片介面沙盒墊上酒館的讀訊息函式，不藏面板；MVU 類另立 [card-mvu-shim](../tasks/card-mvu-shim.md)〔作者裁決 2026-10-02〕。
- 沒重構的卡，酒館怎麼做就怎麼做，不為省工放棄或縮水；只有大改前重構過的桌可以不管〔作者裁決 2026-10-02〕。
- 重構判 playable: no 的 interface 桌改用原卡畫面：拿掉 refactor-noshell-panel 加的「interface＋無骨架就回 null」短路，開回卡片殼的 fallback。refactor-noshell-panel 的清殼、收據、undo 不動〔作者裁決 2026-10-02〕。
  - 這類桌只驗開場白那一樓；後續回合有值要等 [refactor-statusbar-skeleton](../tasks/refactor-statusbar-skeleton.md)，本案不宣稱後續回合可用。
  - 「不接管時保留介面來源條目」已作廢〔作者裁決 2026-10-02〕。
- `interface-card-panel.md` 的「不做：ST 外掛 API 相容」改寫為「目標與酒館行為一致」〔作者裁決 2026-10-02〕。
- 讀訊息類照官方型別規格與酒館實作行為支援，不縮成只回本樓〔模型判斷·未裁決，Sol 第 2 輪同意方向〕：
  - `getChatMessages`：單一樓號、負數深度、範圍字串、三個篩選選項。
  - `getCurrentMessageId` 回本樓；`getLastMessageId` 回最後一樓。
- 對外說法是「這三支函式的本場讀取支援」，不宣稱完整的酒館讀訊息相容。
- gm_only 的系統事件（角色私設、非公開人物全文）不進交給沙盒的逐字稿，也不拿來畫殼〔模型判斷·未裁決，主線決定〕。理由：酒館聊天紀錄裡沒有這類 app 內部資料，排除才符合酒館行為。
- JS-Slash-Runner 只當規格書讀，禁止抄碼（授權限制，見 interface-card-panel.md 參考資料段）。

## 規格（轉述，2026-10-02 讀 main 分支的型別檔與實作）

- **`range`：**
  - 轉成字串，代換 `{{lastMessageId}}`。
  - 只認兩種格式：單一數字 `^-?\d+$`，或範圍 `^-?\d+--?\d+$`（兩端都可為負數深度）。
  - 負數轉成 `最後樓號 + 值 + 1`。反序自動排序，兩端夾進 `[0, 最後樓號]`，所以單一樓號 999 會回最後一樓。
  - 其他格式回空陣列；沒有任何樓也回空陣列。
- **`options`：**
  - `role`：all／system／assistant／user，預設 all。
  - `hide_state`：all／hidden／unhidden，預設 all。
  - `include_swipes`：預設 false。
- **一般回傳形狀：** `message_id`、`name`、`role`、`is_hidden`、`message`、`data`、`extra`，另附相容欄位 `swipe_id`、`swipes`、`swipes_data`。
- **`include_swipes: true` 的回傳形狀：** `message_id`、`name`、`role`、`is_hidden`、`swipe_id`、`swipes`、`swipes_data`、`swipes_info`，沒有 `message`／`data`／`extra`。
- **排序：** 依樓號升冪。
- **`getCurrentMessageId`：** 回所在樓號；`getLastMessageId` 回最後一樓樓號。

沒有刻意偏離：null／undefined 照酒館拋 TypeError；不符格式的字串（NaN、Infinity、小數等）回空陣列；符合格式但超長的整數字串轉成 ±Infinity 後照常夾回，正數落最後一樓、負數落第 0 樓（Sol 驗收時對照酒館參考行為核對）。

## 做法（已實作）

### 1. 樓與本樓（`card-chat-shim.ts`、`card-shell-route.ts`）
- **樓：** 本場 events 排除 gm_only 後的位置就是樓號。
- **本樓：** `pickCardShell` 回 `{ shell, current: { id, name, text } }`，`current` 是產生這份殼的那一樓。
  - 近十則先帶上原始樓號，再 slice／filter／reverse；`findShell` 只回候選序號，由呼叫端對回樓號。
  - 空桌比照酒館「開場白是第 0 樓」，`name` 取命中那張 opening 的卡名。
  - 骨架路徑的本樓 `text` 是填值後的合成文字，這是唯一例外。
- **`-1`：** 照規格是最後一樓，可能是玩家，不是面板那一樓。
- **欄位對應：**
  - `role`：玩家→user；system 事件→system；GM 旁白與角色對話→assistant。
  - `name`：`speaker_name`。
  - `is_hidden`：false。app 沒有隱藏樓，所以 `hide_state: hidden` 回空陣列。
  - `message`：`raw ?? text`。
  - 給不出真值的欄位：`data`／`extra` 為 `{}`、`swipe_id` 為 0、`swipes` 為 `[message]`、`swipes_data` 與 `swipes_info` 為 `[{}]`。
- **`buildCardChat(events, current)`：** 掛載與推送共用。本樓一律覆蓋成產生殼的那段文字，玩家新增一樓後推送也不會把骨架本樓換回原文；`currentId` 一直指向產生目前殼的那一樓。

### 2. 墊片（`card-chat-shim.ts`、`interface-card.ts`）
- `buildShellDocument(shell, seed, { chat, token } | null)`：讀訊息墊片排在橋接墊片之前，在任何誘餌覆寫與卡片 script 執行前，把真 parent 收進閉包。null 時三支都不定義。
- 快照直接寫成 JS 物件字面值，不再 JSON.parse；`<`、`>`、`&`、U+2028、U+2029 轉 `\u` 跳脫。回傳時深拷貝。
- **range 解析：**
  - 只用兩個正規式；null／undefined 先拋 TypeError。
  - 範圍先夾進現有樓層再逐樓掃，超長端點（±Infinity）也只掃現有樓。
- **收推送：** 同時驗 `event.source === parentRef`、`source`／`kind`、token，以及快照形狀（`currentId` 為整數；每樓 name／message 為字串、role 在三值內），不符就忽略。
- 沙盒不能要資料、不能寫回；宿主收訊種類維持 `input`／`storage`／`close`。

### 3. 掛載與推送生命週期（`useCardInterfaceController.ts`、`CardInterfaceOverlay.tsx`）
- **controller：**
  - `chat`＝`buildCardChat(events, current)` 的 memo，存一份到 `chatRef`。
  - `shellKey`＝[worldId, shell, current.id, current.name, current.text] 的指紋，同時當推送 token。
  - `shellDoc` 依賴 shell、shellKey、worldId、uiOpen；只在面板開著時建，建的當下從 `chatRef` 嵌入最新快照。
- **面板關著：** 覆蓋層整支卸載。別樓有變動後重開時，doc 依 uiOpen 重建，卡片第一次執行前就讀得到最新快照。
- **覆蓋層：**
  - iframe 掛 `ref`。
  - `useEffect([chat, shellKey])` 推送一次：本樓沒變、只有別樓變時，資料靠這條進去。
  - `onLoad` 再推一次，補上 doc 建好到殼初始化之間漏掉的更新。第一次讀取靠嵌入的快照，不只靠 load。
  - 只送到目前 iframe 的 contentWindow（opaque origin 只能用 `"*"`）。
- **切桌：** key 含桌別，必然重掛；舊殼收到新桌推送時 token 不符，不收。
- **重掛頻率：**
  - 原本同殼換樓不會重掛，這是讀本樓的卡看不到新值的原因之一。
  - 現在 key 含本樓：本樓換樓或換文就重掛，每回合 GM 回覆一次。
  - 只有玩家送出、狀態樹或其他無關 render 時，doc 與 key 不變，只推送。

### 4. B 項
- 刪掉 `card-shell-route.ts` 的 interface＋無骨架短路，interface 桌沒骨架時照原卡畫面。
- 更新舊測試：選路矩陣改斷言 fallback；`refactor-undo-flow.test.tsx` 改成復原前就有介面鈕。

## 已知限制

- 樓號是本場索引，不是整串聊天的樓號；換幕後從 0 重數，前幾幕的樓讀不到。
- 沒有 swipe、沒有隱藏樓，相關欄位填固定值。
- `getMessageId(iframe_name)` 這類依賴酒館 iframe 命名的函式不墊。
- 本樓原文與逐字稿在沙盒裡，殼載入的 CDN 腳本讀得到；不宣稱與外部網路隔離，這是卡片在酒館裡本來就有的狀態。
- 「面板開著時 GM 新回覆自動換值」零額度只能用收回／復原驗本樓變動；真回覆要低階模型跑一回合，列入實測佇列。

## 測試（已補）

- `card-chat-shim.test.ts`：
  - `buildCardChat`：角色對應、gm_only 排除、本樓覆蓋、空桌、骨架推送後保留。
  - 三支函式：樓號、數字字串、負數深度、範圍（一般、反序、負數端點、巨集、超出夾回、巨大端點）。
  - null／undefined 拋 TypeError；超長整數夾回；10 種不符格式的 range 回空陣列；篩選與非法選項、兩種回傳形狀、回傳物件隔離。
  - 推送驗來源、token、kind、形狀，以及推送後的資料隔離。
  - 仿讀本樓卡片寫法；跳脫 round-trip（引號、反斜線、大小寫混用 `</ScRiPt>`、U+2028／2029、emoji）；墊片順序；沒有快照時不定義。
- `card-shell-route.test.ts`：
  - null 桌與 interface 無骨架（null／空字串／純空白）× 空桌／最新一則／近十則的原卡畫面與本樓。
  - 有效骨架各路徑；骨架合成文字。
  - 樓號：混合事件、gm_only、重複原文、近十則邊界、多卡 opening。
  - characters／未知模式。
- `useCardInterfaceController.test.ts`：
  - 面板跟著殼關閉。
  - 同樓換文時 key 變；切桌時 key 變。
  - 玩家新樓不重掛只更新快照；無關 render 時 doc 與 key 不變。
  - 面板關著時別樓更新，重開後 doc 含最新快照。
  - 收回時本樓回退；骨架本樓在推送後保留。
- `CardInterfaceOverlay.test.tsx`：還沒 load 就有更新、load 補推最新；新 key 換新 iframe 且只帶新 token；沒有快照不推。
- `refactor-undo-flow.test.tsx`：依 B 項更新斷言。

## 驗收（GUI，零額度）

用 test-harness，獨立 root，不啟動正式包：
1. **NorthHall-structure 空桌：** 開場白自帶 `<Status_block>`，打開面板要顯示開場白的值。
2. **兩組假 GM 訊息：** 用 `invoke post_opening` 寫兩則，各帶卡片實際 YAML 欄位的一組不同值。回大廳再進桌讓前端重讀，打開面板要顯示第二則的值。
3. **收回與復原：** 面板開著，走前端「收回上一句」→ 面板自動換成第一則的值；「復原剛收回的」→ 換回第二則。記錄重掛次數、iframe 掛上到 load 的時間（CDN 恢復時間）、快照資料量；展開狀態欄等無關操作不重掛。
4. **RPGImmortal：** 用它的 YAML 欄位驗兩組值。
5. **B 項：** 重構判 playable: no 的 interface 桌（套 nhs 產物），空桌打開面板顯示開場白那一樓。
6. ai-log 零派送；quit 後兩個正式目錄 hash 不變。

CDN 載不到時，視覺驗收記成待完成，不改做法。

### 實測結果（2026-10-02，test-harness，零 AI 派送，正式目錄 hash 不變）
- NorthHall-structure：
  - 空桌顯示開場白那一樓的值。
  - 兩則假訊息後顯示第二則的值。
  - 面板開著按收回，自動換成第一則；按復原，換回第二則。每次重掛 1 次，掛上到 load 約 8 ms（CDN 已快取）；srcdoc 約 16 KB，逐字稿快照約 1 KB。
  - 展開狀態欄等無關操作重掛 0 次。
- RPGImmortal：兩組值都顯示，收回後換值；重掛 1 次，掛上到 load 122 ms，殼本身約 336 KB。
- nhs 產物（playable: no）套在新的 NorthHall-structure 桌：mode=interface、沒殼，空桌顯示開場白那一樓的值。
- 截圖：主線 scratchpad `ccm/shots/`（1～7）。
