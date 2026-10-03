# card-mvu-shim — 計畫

交接：[handoffs/card-mvu-shim.md](../handoffs/card-mvu-shim.md)　先例：[card-chat-messages-shim](card-chat-messages-shim.md)

## 定案範圍

- 卡片介面沙盒墊上 MVU 前端卡讀變數用的函式，讓 MVU 卡在 app 裡畫得出值；2026-10-03 開工〔作者裁決 2026-10-02〕。
- 沒重構的卡照酒館行為做，不藏功能省工〔作者裁決 2026-10-02，見 memory「酒館行為照做」〕。
- 寫入類 API（`Mvu.setMvuVariable`、`Mvu.replaceMvuData`、`insertOrAssignVariables`、`replaceVariables` 等）：照酒館語意實作並寫回 app 狀態樹（A3），宿主端做信任邊界驗證〔作者裁決 2026-10-03〕。理由：沒重構的卡照酒館行為做，不藏功能。
- 分兩包，同案做完〔作者裁決 2026-10-03〕：包 1＝只讀墊片（第 0–7 節，Sol 第 4 輪同意）；包 2＝寫入（第 8 節，以 Sol 第 2 輪 10 項必改為規格重寫後再送審）。
- 非 message 層變數（chat／character／global 等）照酒館做（B2），歸包 2；包 1 期間這幾層讀取的暫時行為由包 1 計畫明列，不暗改預設層〔作者裁決 2026-10-03〕。
- 歷史樓寫入照上游語意：允許改該樓快照、不覆寫目前狀態，歸包 2〔作者裁決 2026-10-03〕。
- 沙盒不得直接存取檔案；既有 localStorage 橋接照舊保存卡片設定。讀取部分的 MVU 推送是宿主→沙盒單向。
- JS-Slash-Runner、MagVarUpdate（MVU）只當規格書讀，禁止抄碼（授權限制，同先例）。

## 受惠卡實際用到什麼（2026-10-03 讀 TestCards 卡內 regex 與 tavern_helper 腳本）

| | bcd368（`mvu前端`、`mvu前端（武侠）`） | DongeonMaster（`迷宫之书`） |
|---|---|---|
| 觸發 | `<StatusPlaceHolderImpl/>`，placement 2、markdownOnly | 同左 |
| 讀值 | `_.get(getAllVariables(), 'stat_data', {})` | 同左，再 `_.get(V, '基础信息.时间')` 等點分路徑 |
| 時機 | `$(errorCatched(init))` → `await waitGlobalInitialized('Mvu')` → 畫一次 → `eventOn(Mvu.events.VARIABLE_UPDATE_ENDED, 重畫)` | 同左 |
| 寫入 | 無（送出走 `window.parent.document #send_textarea`，既有誘餌已接） | 無（送出同左；本地資源存 localStorage，既有墊片已接） |
| 值的型別 | 一律 parseInt／parseFloat，容得下字串 | `cleanGet` 一律 `String(v)`，自己濾 `$meta` 鍵 |
| 依賴全域 | `$`（jQuery）、`_`（lodash）、`errorCatched`，卡內不自載 | 同左 |

- 兩張卡的 `[initvar]` 都是一般值；DongeonMaster 帶 MVU 的 `$meta`（extensible／template）與 `[]` 空陣列。
- tavern_helper 腳本：`MVU基础脚本`（import MVU bundle）與 `变量结构`。app 不執行這些腳本；變數由 app 狀態樹管（state-values-mvu）。
- 開場白：bcd368 的 first_mes 帶占位；DongeonMaster 的 first_mes 沒占位，開場畫面走另一支 `开局` regex。

## 做法

### 0. 前提：沙盒內建全域庫〔模型判斷·未裁決〕
酒館助手的 iframe 內建 jQuery、lodash 與 `errorCatched`，兩張卡都直接用、不自己載；沙盒現在沒有，`$(errorCatched(init))` 第一行就拋錯。本案一併墊上：
- 新增依賴 `jquery`、`lodash`，`package.json` 鎖精確版本；以 Vite `?raw` 讀 min 檔內嵌進 srcdoc（酒館是本地提供，離線也要能畫）。每份 srcdoc 約多 160 KB。驗證正式 build（`npm run build` 產物裡的 srcdoc 確實含兩個庫）。
- 一律注入（酒館助手也是一律有）。卡片自己再載一份庫會覆蓋全域，相容性不保證，列測試驗「重載庫後 `$`／`_` 仍可用」。
- `errorCatched(fn)`（照酒館助手 `src/function/util.ts`）：同步例外通報後照樣拋出；回傳 Promise 時改回傳「通報後再拋」的接續 Promise。沙盒沒有酒館通知（toastr），通報改記 `console.error`。`this` 照原呼叫傳下去（上游是箭頭函式不轉傳，這裡多保留不影響一般呼叫）。

### 1. 狀態來源（每一樓的 stat_data）
- **每樓的來源狀態**：該樓之前（含）最近一則帶 `state.tree` 的事件快照——對照 MVU「每樓存自己回覆完的變數」。
- **前提**：每則事件落檔時都會蓋上當下狀態快照，玩家句也一樣（`append_player`／`append_transcript` 的 `stamp_state`，`src-tauri/src/commands/scene.rs`）。所以「最後一則帶快照的事件」幾乎永遠是最後一樓，不能拿它當活／歷史的分界：玩家尾樓會變成分界，最新那支 GM 殼反而落在分界前，收不到手改值。
- **會改狀態的樓**：只有 `post_opening`（開場）與 `gm_narrate`（GM 回覆）會套用狀態更新（`mechanism::apply_block` 只在這兩處呼叫）；玩家句、角色台詞、一般系統事件只是蓋上當下快照。辨識：兩者都落成 `kind: "narration"`、`speaker_id: ""` 的事件（`append_opening`、`useChatController` 的 GM 旁白）；中止或截斷的 GM 回覆（`truncated: true`）也是同一形狀，一併算會改狀態的樓——中止回覆後面若沒有新 GM 樓，它自己就是活樓、照樣讀目前狀態。
- **活的那一份**：本樓之後沒有任何「會改狀態的樓」→ 本樓用目前狀態 `tableState.tree`（含面板手動改值）。最新 GM 殼後面接著玩家句、角色台詞都還是活的。理由：`set_state_path` 改的是磁碟上的最後事件與 state.json，前端 `chat.events` 不重讀（`useTableStateController.ts` 的 `refresh()` 只更新 `tableTree`），只讀事件快照收不到手動改值。
- 一則帶狀態的事件都沒有（空桌、只有開場）：全部樓用 `tableState.tree`。
- **歷史殼**：本樓之後還有「會改狀態的樓」→ 只用本樓之前（含）最近一則快照，絕不混入後面樓的狀態或手動改值。
- 每樓資料（`getMvuData` 指定樓號時）用同一套判斷：該樓之後沒有會改狀態的樓就用目前狀態，否則用快照。
- 接線：`App.tsx` 把 `tableState.tree` 傳進 `useCardInterfaceController`。

### 2. 狀態樹 → stat_data（純函式 `buildMvuData`）
- **形狀**：`{ stat_data, display_data, delta_data }`。`stat_data`＝轉換後的樹；`display_data` 給 `stat_data` 的深拷貝；`delta_data` 給 `{}`（app 沒存舊值，見已知限制）。
- **葉子型別還原**（樹的葉子一律字串）：
  1. `mechanism.value_types` 命中該路徑時照它，名稱是 `number`／`bool`／`list`：
     - `number`：符合下方數字格式且 `Number()` 有限才轉，否則保留字串。
     - `bool`：只認 `true`／`false`，否則保留字串。
     - `list`：照規則 2 試 JSON 陣列，不成功保留字串，不猜分隔符。
  2. 沒有型別記錄時：
     - 數字：`^-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?$` 且 `Number()` 有限才轉；溢位成 ±Infinity、前導零（`007`）、`.5`、`1.`、`+1` 一律保留字串。結果不會出現 NaN／Infinity，深拷貝不會變 null。
     - `true`／`false` → boolean；`null` → null。
     - 以 `[` 或 `{` 開頭且 `JSON.parse` 成功 → 陣列／物件（`[]`、`[1, "說明"]`）。只還原合法 JSON；YAML 行內寫法（`[1, 说明]` 未加引號）保留字串。
     - 還原後遞迴檢查：物件／陣列裡任一數字不是有限值（例如 `[1e999]` 解析成 `[Infinity]`，深拷貝會變 `[null]`），整個葉值保留原字串。
     - 其他一律字串。不用 js-yaml 整串解析：`#烦躁 #废了` 這類被剝掉引號的字串會被當成註解變 null。
- 舊式 `[值, 說明]`：`stat_data` 保留整個陣列（與上游一致）。
- `$meta` 每一層都清掉（含 JSON 還原出的巢狀值）：上游初始化時 `cleanUpMetadata(stat_data)` 會移除，stat_data 裡本來就沒有。
- **`{{user}}`／`{{char}}`**：上游在解析 initvar 前整段 `substitudeMacros`，所以鍵與值都換（不分大小寫；`{{char}}` 換成載入 MVU 那張卡的名字）。一次掃完、替換值當字面（名字裡的 `$&` 不當替換語法），整份資料只清理代換一次。鍵代換後與同層既有鍵撞名時，該鍵保留原字面不換並 `console.warn`〔模型判斷·未裁決〕。
- 資料鍵一律存成自有屬性（`__proto__`、`constructor` 也是普通鍵）；沙盒端快照以 `JSON.parse` 載入，內部表用 Map。

### 3. 墊片（新檔 `card-mvu-shim.ts`）
沙盒內定義（排在讀訊息墊片之後、橋接墊片之前，真 parent 一樣先收進閉包）：
- `getAllVariables()`：回本樓 MVU 資料深拷貝。酒館是全域→角色→聊天→各樓依序頂層 assign，MVU 每樓存完整 stat_data，等於本樓那份整份取代；app 只有 message 層（其他層包 2 補）。
- `initializeGlobal(name, value)`／`waitGlobalInitialized(name)`（照酒館助手 `src/function/global.ts`）：上游就是 `_.set(window, …)`／`_.has(window, …)`，直接用沙盒內嵌的 lodash（墊片初始化時先收好，卡片換掉 `window._` 也不影響），路徑語意（括號、索引、既存字面鍵）照 lodash。`initializeGlobal` 設值後發 `global_<名稱>_initialized`；`waitGlobalInitialized` 已存在就立即 resolve，否則等那個事件。兩者都不回傳值、不逾時。墊片用 `initializeGlobal('Mvu', Mvu)` 分享 Mvu。`getMvuVariable` 取值同樣用這份 lodash 的 `_.get`。
- `Mvu.events`：`VARIABLE_INITIALIZED`、`VARIABLE_UPDATE_STARTED`、`SINGLE_VARIABLE_UPDATED`、`VARIABLE_UPDATE_ENDED`，字串值照上游。
- `Mvu.getMvuData({ type: 'message', message_id })`：照樓解析，不把別樓偽裝成本樓：
  - 非負整數：該樓的資料（第 1 節的規則）。
  - 負數：照讀訊息墊片的深度換算（最後樓號＋值＋1）。
  - `'latest'`（也是不給 `message_id` 時的預設）：最後一則**非 system** 樓的資料（酒館助手 `src/function/variables.ts`）；明確給 `-1` 才是最後一樓（可能是 system）。
  - 超出 `[-樓數, 樓數)` 拋錯（酒館助手 `getVariables` 規格）。
  - `type` 不是 `message`、或沒給 `type`：照待拍板 B 的裁決，不暗自把預設層當成 message。
- `Mvu.getMvuVariable(data, path, { category, default_value })`：category 選 `stat_data`／`display_data`／`delta_data`（預設 stat）；取到長度二、第二項是字串的陣列時回第一項（上游 `src/function/global/index.ts` 的行為）；取不到回 `default_value`。
- 事件函式（照酒館助手 `src/function/event.ts`）：`eventOn`／`eventOnce`（回 `{ stop }`、同一函式不重複註冊）、`eventRemoveListener`、`eventClearEvent`／`eventClearListener`／`eventClearAll`、`eventMakeFirst`／`eventMakeLast`（已註冊只搬位置、保留原註冊的 once；未註冊就新註冊在最前／最後）、`eventEmit`。發事件時監聽器依序一支等完再下一支；派送途中被移除的監聽器輪到時不再呼叫；單支拋錯或拒絕記錄後照樣通知其他支。參數原樣傳（`eventEmit` 可傳函式、循環參照）；只有宿主推送的 MVU 事件每支拿自己的快照深拷貝，改了不污染快照。
- 快照形狀：每樓資料去重成 `states: Data[]`，再用 `floorState: number[]` 對回每樓，避免同一份樹每樓重複。

### 4. 事件時機
- 掛載：快照嵌進 srcdoc，卡片 init 時 `getAllVariables` 直接拿到值；`waitGlobalInitialized('Mvu')` 立即 resolve。
- GM 新回覆：本樓換了 → 既有機制整支重掛 → 卡片 init 重讀，不需事件。
- 不重掛的變動（本樓是活的那一份、玩家在面板手動改值）：沿用讀訊息的推送通道，快照多帶 `mvu` 欄。墊片收到後：
  1. 本樓資料與現有深比較，相同就只更新其他樓的資料、不發事件。這是 app 的推送規則；上游即使沒變動也會發事件，本案不重現，因為推送時機是 app 自己的（例如 onLoad 補推），照發會讓卡片初始化時重畫兩次。
  2. 不同：先發 `VARIABLE_UPDATE_STARTED`，只傳當下（舊）資料、等監聽器跑完；再換掉 store；對每個變動的葉子（含整個分支新增或刪除）發 `SINGLE_VARIABLE_UPDATED`（`stat_data, path, 舊值, 新值`）；最後發 `VARIABLE_UPDATE_ENDED(新資料, 舊資料)`。參數順序已對照上游 `update_variables.ts`。事件字串照上游 `variable_def.ts`（`mag_variable_update_ended` 等）。
  3. 推送排進佇列，前一筆的事件全部跑完才處理下一筆，多次推送不交錯。
  4. `VARIABLE_INITIALIZED` 不發：app 在殼掛載前就已初始化好變數。
- 推送驗證照先例：`event.source === parentRef`、token、kind、形狀（`states` 每份的 `stat_data` 為物件、`floorState` 長度等於樓數且索引合法）。

### 5. 占位與殼何時出現
- 上游行為（`update_variables.ts` 的 `handleVariablesInMessage`）：處理收到的 AI 樓（第 0 樓也照處理，沒有開場白時第一樓就是 AI 回覆）；內容短於 5 字、沒有 stat_data 就不處理；處理後該樓不是玩家樓、內容沒有占位，就在樓尾接 `\n\n<StatusPlaceHolderImpl/>`。開場白由 initvar 初始化（`variable_init.ts`），那段不補占位。
- app 比照：這桌有卡的酒館助手腳本載入 MVU（後端 `CardInterface.mvu`，啟用中、內容含 MagVarUpdate 的腳本）、沒走重構骨架、該樓是 assistant（GM 旁白或角色台詞；app 的系統事件不算）、不是開場白、內容至少 5 字、該樓 stat_data 非空、沒有占位，才在樓尾補。開場白靠事件的 `opening` 標記認（後端 `append_opening` 寫入時標上，舊紀錄沒有這欄）。
- 補好的文字只算一份：交給 regex 選殼與 `getChatMessages` 的是同一份，不重複補。
- DongeonMaster 開場：`extractShell` 會把同一段文字裡所有 html 區塊接在一起，開場若同時有 `<开局>` 和補上的占位，兩份殼會黏成一份壞文件。開場白不補占位，所以「开局」畫面不會被搶走（單元測試已涵蓋，GUI 驗收再看一次）。

### 6. 驗收門檻：GM 格式指示〔Sol 第 1 輪要求〕
`gm_narrate` 在「有可渲染卡片介面」的桌會把導演指示換成 `card_format_instruction`，要求只照卡片規定格式輸出。兩張卡都有卡片介面，需要確認：
- 對照兩卡世界書的 `[mvu_update]` 輸出格式條目，與實際送出的 GM 提示（harness 抓 prompt，零額度）；確認 `<UpdateVariable>` 規定仍然送到、沒有被「只吐 XML／不要 state」的指示壓掉。
- 若衝突會擋掉變數更新：另立案處理；本案結案時只宣稱開場與手動改值可用，不宣稱後續回合可用。

### 7. 檔案落點
- 新增 `src/features/card-interface/card-mvu-shim.ts`（＋test）：`buildMvuData`、型別還原、每樓狀態對應、墊片原始碼。
- 新增 `src/features/card-interface/card-sandbox-libs.ts`（＋test）：jQuery／lodash raw 內嵌與 `errorCatched`。資料夾 11 → 15 檔。
- 改前端：
  - `interface-card.ts`：`buildShellDocument` 串墊片。
  - `card-chat-shim.ts`：推送快照加 `mvu` 欄與驗證。
  - `card-shell-route.ts`：占位補法，同一份文字給選殼與讀訊息。
  - `useCardInterfaceController.ts`：每樓來源、活狀態、value_types、玩家名。
  - `CardInterfaceOverlay.tsx`：推送內容。
  - `src/App.tsx`：傳 `tableState.tree`。
- 改後端：`src-tauri/src/import/interface.rs`，`CardInterface` 加 `mvu`（卡的 tavern_helper 腳本是否載入 MVU）。前端型別同步。
- 包 2（第 8 節）：
  - 前端新增 `card-mvu-write.ts`（宿主端驗證、依完整目標的佇列與 Promise 結算）、`card-mvu-parse.ts`（parseMessage），各附測試；沙盒寫入函式加在 `card-mvu-shim.ts`，超過 1000 行就把沙盒原始碼拆到 `card-mvu-shim-source.ts`。資料夾約 21 檔，屆時考慮開 `card-interface/mvu/` 子資料夾。
  - 後端：短提交鎖與投影入口放 `data/` 層（`world_lock.rs` 旁、`data/state.rs`）；`commands/card_vars.rs` 只做邊界；非 message 層檔案讀寫放 `data/card_vars.rs`；`TranscriptEvent` 加 `id`、`message_vars`、`vars_rev`。

### 8. 包 2：寫入、非 message 層、parseMessage（2a 已施工，2b、2c 未施工）

規格來源：作者裁決「卡片變數只存一處」（定案範圍）、Sol 第 10、11 輪；上游釘版本——MagVarUpdate `438f9ffc`（2026-10-01）、JS-Slash-Runner `46ec10df`（2026-10-01）、SillyTavern release `06bde939`（2026-09-14）。標〔Sol〕的是 Sol 對上游的讀法；施工第一步照釘住的版本逐行核對，有出入停下回報。

分包（C2）：2a 寫入核心（8.1–8.6、8.8）→ 2b 非 message 層（8.7）→ 2c parseMessage（8.9）。2a 期間非 message 層維持包 1 的「明確拋未支援」；2a 全部驗收完才進 2b。

#### 8.1 資料模型
- `TranscriptEvent` 新增：
  - `id`：穩定 ID（ULID），新事件落檔時由後端配發。舊事件沒有 ID：以「場別＋序號＋ts＋整則內容指紋」定位，第一次被寫入時在同一次改寫裡補上。
  - `message_vars`：該樓完整 MVU 變數表（JSON，保留型別、缺鍵、`display_data`／`delta_data`、`schema`、自訂鍵）。`None`＝這樓尚無表；`Some(物件)`＝明確的表（可以沒有 `stat_data` 鍵）。
  - `vars_rev`：版本 token（ULID），每次這樓的表變更、或收回後復原帶回這則事件時都重新產生，不由現存最大值推算，刪除後也不會撞號。
- 每張表另記 `vars_epoch`（寫在事件上，與 `message_vars` 同一次落檔）＝寫入當下那一幕的 epoch。
- **控制檔** `worlds/<id>/card-vars/control.json`（單一檔、整檔原子寫，是模式與種子的唯一權威）：`{ mode: events | tree, scenes: { 場別: { epoch, seed: 完整表 } } }`。
  - 每一幕各有自己的 `epoch` 與種子。模式交接只換「目前這一幕」的 epoch 與種子，其他幕原封不動；所以子幕做過重構往返再退回父幕，父幕的表仍是有效的。
  - 初始化來源只認 `vars_epoch` 等於該幕 epoch 的表；舊 epoch 的表仍留在事件上，逐樓 API 照樣讀得到自己的表。
  - 種子＝那一幕開頭的完整表（含 `schema`、自訂鍵、`initialized_lorebooks` 等），獨立存放、不被快取重建覆寫。initvar 匯入、開桌初值寫進目前那一幕的種子。
- **模式交接**（都在短提交鎖內、可重試、看目前 mode 決定要不要做，所以重做無害）：
  - 啟用（`tree` → `events`）：有 MVU 卡、沒有重構骨架的桌，第一次 GM／開場提交或卡寫時做。目前這一幕換新 epoch；它的種子＝上一份完整表（沒有就空表）沿用其他鍵、`stat_data` 換成當下 `state.json` 樹的物化值（與上一份同路徑同值的沿用原 JSON 值，其餘照包 1 規則還原）、`display_data`／`delta_data` 重新衍生；一次寫入控制檔即發布。舊桌首次啟用同此；觸發啟用的那筆卡寫接著在新 epoch 下寫它的目標（含尾樓 E），所以不會被排除。
  - 重構套用（`events` → `tree`）：先把當下有效 `stat_data` 投影寫進 `state.json` 樹並確認寫成，再寫控制檔 `mode: tree` 發布。中途崩潰時控制檔仍是 `events`，有效狀態照舊從事件來，重做即可。
  - 重構復原／重置回原卡（`tree` → `events`）：同「啟用」，目前這一幕以當下 `state.json` 樹物化新種子、新 epoch。重構期間的修改因此保留，不會讀回重構前的舊表。
  - 控制檔寫入失敗：交接整個不生效（模式不變），回錯；`events` → `tree` 時先寫的樹只是快取，不影響 `events` 模式的讀取。
  - 停用腳本、讀不到卡片介面都不改模式。
- **桌世代**：記憶體內每桌一個世代號，整桌交換／還原（8.4 清單）時加一。寫入請求都帶世代號，不符一律 `stale`；備份還原帶回相同事件 id 與 token 也會被擋。app 重開時宿主佇列本來就清空，世代號不必落檔。
- 不另開每樓變數檔；卡寫只改目標事件那一行（同檔鎖內整檔讀改寫）。2a 先量測長逐字稿（500 則、每則帶 25 KB 表）的讀取與整檔改寫時間，超過 100 ms 停下回報。

#### 8.2 讀取：逐樓 API 與初始化來源分開
- **逐樓 API**（沙盒 `getVariables`／`getMvuData` 的 message 層）：讀該樓自己的 `message_vars`，明確表原樣回傳（缺 `display_data`／`delta_data` 就是缺）；沒有表回 `{}`。不往前繼承。
- `getAllVariables()`：全域 → 角色 → 聊天 → 0 樓到本樓各 message 層依序頂層 `_.assign`，所以沒有表的樓會帶出前面樓的值（酒館同此）。
- **初始化來源**（MVU `getLastValidVariable`）：只看目前這一幕——依位置最新一則 `vars_epoch` 等於這一幕 epoch 的帶表事件；沒有就用控制檔裡這一幕的種子。不掃其他幕，所以換幕、退幕、分岔都不會讀到已離開的幕。只給 GM／開場建新表、手改與既有機制（面板狀態欄、GM 提示、`apply_block`）用，不給逐樓 API。
- **目前有效狀態**＝初始化來源。`read_state` 改成投影入口：`events` 模式時把 `tree` 換成有效 stat_data 的投影（只在給既有機制時轉成字串葉子）；原本直接讀檔改名 `read_state_cache`，只給入口與重建用。入口有 tx 版，鎖內呼叫不重取鎖。
- `state.json` 的 `tree` 在 `events` 模式只是快取：提交後依入口重寫，失敗不影響提交，下次重建。
- 衍生的 `display_data`（stat_data 拷貝）、`delta_data`（`{}`）只在建立新表時產生（GM／開場提交、從種子建第一張表），讀取時不補。
- 推給沙盒的快照每樓帶：目標定位（世代、場、事件 id）與該樓表版本（`vars_rev` 或「無表」）；另帶初始化來源的事件 id 與版本，給 `getAllVariables` 與一致性檢查用。

#### 8.3 寫入路徑（`events` 模式）
- **卡寫（message 層）**：目標＝樓號解析出的事件（讀取 `'latest'` 排除 system、`-1` 不排除；**寫入 `'latest'` 取最後一樓含 system**〔Sol〕）。目標身分與讀值來源分開：寫入沒有表的 B，目標就是 B、預期版本「無表」，不因 B 的 `getAllVariables` 讀值來自 A 而寫到 A。鎖內驗 `{ 世代, 場, 事件 id, 預期版本 }`，相符才把整張新表寫進那則事件、產生新 token；不動種子與 `state.json`（快取另外重建）。寫哪一樓就只影響那一樓；它成為有效狀態只會是因為位置最新。
- **GM 回合**（所有階段都核對 `turn_id`，不符一律拒絕且不改狀態）：
  - 開始（鎖內）：記回合紀錄 `{ turn_id, 階段: 生成中, 固定輸入＝有效狀態＋來源版本 }`。這段卡寫與面板手改回 `busy`〔模型判斷·未裁決：手改也擋，避免被回合結果蓋掉〕。
  - 模型等待期間不持鎖。
  - 提交（鎖內、核對 `turn_id`）：`apply_block` 套在固定輸入上，新表＝來源表，`stat_data` 裡 GM 改到的路徑換新值（字串照包 1 規則還原型別）、沒改到的保留原 JSON 值，`display_data`／`delta_data` 重新衍生。放進回合紀錄，階段改「等落檔」。
  - 落檔（鎖內）：前端照舊 `append_transcript`，帶 `turn_id` 與 `turn_part`。`turn_part` 固定幾種：`main`（GM 正文，回合的主事件，唯一會掛表的一則）、`state_update`（變動紀錄系統事件，附屬、永遠不掛表）等。`main` 在「等落檔」時把紀錄裡的表掛上事件、不收前端傳來的變數；`main` 一寫成就轉「已落檔」並解除 `busy`，卡寫與手改從此放行。
  - 冪等鍵＝`(turn_id, turn_part)`，同時寫進事件本身（`turn_key` 欄，舊事件沒有）。同鍵重複呼叫（前端重試）時，先查紀錄、再查本幕逐字稿尾端有沒有同 `turn_key` 的事件，有就回那則、不再追加——寫檔成功但回傳失敗的重試也認得出已提交。不同 part 各自處理，不會全回正文。寫檔失敗沒有事件，重試照常。
  - 中止：後端中止路徑不套狀態、不產生表，紀錄改「已中止」；前端保留半截正文時以 `main` 落一則不帶表的事件，同樣冪等。
  - 紀錄保留到這桌下一個回合開始、整桌交換／還原或 app 重開才清，期間重試都冪等。錯誤與換桌清除都核對 `turn_id`。
- **開場**：`append_opening` 以初始化來源為底套開場狀態塊，事件帶新表。
- **面板手改**（`set_state_path` 等）：改初始化來源——這一幕目前 epoch 最新帶表的事件（新 token），沒有就改這一幕的種子（寫控制檔）；舊值是字串就存字串，否則照包 1 規則還原型別。
- **收回**：移除事件連同表，初始化來源自然退回；收光這一幕所有帶表事件就回到這一幕的種子（不是快取）。**復原**：帶回完整事件與表，`vars_rev` 換新 token。
- **整表 replace**：照上游存整張表；缺 `stat_data` 就存成沒有 `stat_data` 的明確表。
- 卡寫上限與驗證見 8.8，任一條不符整批拒絕。

#### 8.4 短提交鎖與遷移
- 每桌一把互斥短提交鎖，集中入口 `state_commit::with_commit(root, world, |tx| …)`；內部函式收 `&CommitTx`，含投影入口的 tx 版，互相呼叫不重取鎖（例如 `append_opening` 用 tx 版 `append_transcript`）。鎖內只做驗證與檔案讀改寫，不等模型。
- 鎖順序：世界許可（共用或獨占）→ 短提交鎖 → 逐字稿同檔鎖。已持獨占世界鎖的操作不再取共用許可。
- 走這把鎖的入口：
  - 逐字稿：`append_transcript`（含 `stamp_state`）、`append_player_event`、`discard_unanswered_player`、`pop_transcript`、復原、`set_last_transcript_state`。
  - 開場：`post_opening`、`append_opening`、`OpeningCheckpoint::restore`。
  - 回合：`gm_narrate` 開始與提交、角色回合開始、`chat_abort`。
  - 幕：`data/scene/lifecycle.rs` 的換幕、退回上一幕、分岔、改快照、改寫摘要。
    - 發布原則：先把新幕需要的資料都準備好，最後才改 `current_scene` 發布；`current_scene` 之前的幕（含目前幕）的種子永遠不被覆寫，還沒發布的新幕資料可以被重試覆寫。
    - 換幕：①算新幕種子＝舊幕結束時的初始化來源（完整表）、新幕新 epoch，寫進控制檔 ②寫新幕開頭的摘要事件 ③寫 `current_scene`。任一步失敗就停、回錯；重試從頭做，新幕號仍是「目前幕＋1」，①② 覆寫上次沒發布的殘留，不會出現「目前幕沒有種子」。
    - 分岔：①複製來源幕事件到新幕逐字稿，重新配發事件 id 與 `vars_rev`、表的 `vars_epoch` 改成新幕 epoch ②控制檔寫新幕種子（沿用來源幕種子、新 epoch）③寫 `current_scene`。失敗處理同換幕。
    - 退幕：①先寫 `current_scene` 切回父幕（父幕事件、epoch、種子原封不動）②成功後才從控制檔移除子幕種子；② 失敗只留下用不到的殘留，下次換幕到同一號時會被覆寫。
  - 卡寫指令（新）、`set_player_card`（新）。
- **變數模式的樹寫入者**（`events` 模式下直接改 `state.json` 樹會被投影蓋掉，逐一改寫權威）：
  - `commands/state.rs` 的 `set_state_path` 與其他改樹的手改：改初始化來源（上一點）。
  - `commands/chat.rs`、`mechanism/ledger.rs` 的 `apply_block`（GM、開場）：改走 8.3 的回合紀錄／開場新表。
  - `data/scene/transcript.rs` 收回／復原時把快照寫回 `state.json` 樹那段（約 253 行）：`events` 模式改成重建快取。
  - `import/mechanism.rs`（角色卡附帶初值、initvar 只補不覆蓋）：`events` 模式只補進初始化來源那一份（有帶表事件就改那則、新 token；沒有就改這一幕的種子），單一檔、單一權威，沒有部分失敗。其他幕的種子不動。
  - `receipts.rs` 匯入復原時移除匯入加進的分支（約 810 行）：從復原當下的初始化來源那一份移除（與匯入同一規則）；匯入後已有新回合時，較早事件與其他幕種子裡的那些分支不追溯。
  - `refactor/apply.rs` 重建狀態欄位：屬 `events` → `tree` 交接（8.1），先交接再改樹。
  - `data/worldbook.rs`、`data/scene/presence.rs`、`data/world.rs` 只改樹以外的欄位，不受影響（施工時再確認一次）。
- **整桌交換／還原**：`restore_world_backup`、格式轉換與還原（`data/format/commit.rs`）、`undo_last_import`、重構套用與重置（`refactor/apply.rs`、`refactor/reset.rs`）、`OpeningCheckpoint::restore`：鎖內清回合紀錄、桌世代加一（8.1）。
- 前端兩處「讀整份 state → 改 `player_card_id` → `write_state`」改成 `set_player_card(world_id, card_id | null)`。
- 不在本案：非變數模式桌既有的「state.json 與逐字稿兩檔」寫法（GM、收回）維持現狀，可另案〔Sol 第 12 輪同意〕。

#### 8.5 宿主與沙盒
- 沙盒寫入函式（8.6）先改本地值（保持上游同步語意），再送 `{ kind: "mvu-write", token, requestId, target, version, payload }`。
- **完整目標**＝`{ 桌, 世代, 場, 事件 id, 層, 身分 }`（message 層身分空；非 message 層見 8.7）。宿主依完整目標各自保存三份：權威值與版本、樂觀值、待送版本。不同目標互不覆蓋、互不代結算。同一目標同時只送一筆；在飛時新寫入只更新樂觀值，回來後以樂觀值為整張新表再送。
- Promise 結算：每個 `requestId` 綁目標與本地版本；該目標某版本提交成功，不晚於它的請求一起 resolve。
- 被拒：宿主先在鎖內一致讀回該目標的權威值與版本，再推回沙盒；在飛那批與其後未送版本一起 reject；沙盒換成權威值後發外部變動事件〔模型判斷·未裁決：上游沒有拒絕〕。
- 換殼、切桌、關面板、token 失效：未結算請求一律 reject（`closed`），不留懸空；在飛那筆後端結果照實落檔，但不推給舊殼。

#### 8.6 上游寫入 API 與 app 對應（message 層，2a）

酒館助手變數函式：

| API | 上游語意 | app |
|---|---|---|
| `getVariables(option)` | 回該層表；預設層 chat | 該層深拷貝 |
| `replaceVariables(vars, option)` | 同步、回 void，整張表換掉 | 本地立即換、送宿主；回 void |
| `updateVariablesWith(updater, option)` | updater 回新表（可 Promise），回新表 | 同上；Promise 版 resolve 後才算本地新值 |
| `insertOrAssignVariables(vars, option)` | `_.mergeWith(舊, 新, 陣列整個取代)`，回新表 | 同 |
| `insertVariables(vars, option)` | `_.mergeWith({}, 新, 舊, 陣列取舊)`：既有值優先 | 同 |
| `deleteVariable(path, option)` | `_.unset`，回 `{ variables, delete_occurred }`（缺路徑也可能 true） | 同，原樣交回 |

MVU：

| API | 上游語意 | app |
|---|---|---|
| `Mvu.getMvuData(option)` | 等於 `getVariables` | 同 |
| `Mvu.replaceMvuData(data, option)` | 直接回 `replaceVariables(...)` | 回 Promise，宿主確認落檔才 resolve、被拒 reject——**app 加強語意**，上游不等落檔 |
| `Mvu.setMvuVariable(data, path, value, { reason, is_recursive })` | 只改傳入的 `data`；舊值是任何長度二的陣列就當 `[值, 說明]` 改第 0 項；**不做 `Number()` 轉型**（在 parseMessage 的 set 分支）；display／delta 只更新 `stat_data.$internal` 那份引用；只有 `is_recursive` 為真才發 `SINGLE_VARIABLE_UPDATED`；回 Promise<boolean> | 照做，純沙盒內運算，不送宿主 |
| `Mvu.parseMessage(message, old)` | 見 8.9 | 2c |

- replace／insert／delete／updateVariablesWith 上游不發 MVU 事件，app 照做。
- `getAllVariables()`：全域 → 角色 → 聊天 → 0 樓到本樓各 message 層，頂層 `_.assign`（2b 前只有 message 層）。

#### 8.7 非 message 層（2b）

| 層 | 酒館存放處 | app 落檔（固定身分，宿主決定） | 收回時 |
|---|---|---|---|
| chat | `chat_metadata.variables` | `worlds/<id>/card-vars/chat.json` | 不倒回 |
| character | 角色卡的 tavern_helper 設定 | `worlds/<id>/card-vars/character/<身分檔名>.json`；身分＝目前殼所屬卡的 `character_id`（世界書卡固定 `world`） | 不倒回 |
| global | `extension_settings.variables.global` | 資料根目錄 `card-vars/global.json`（跨桌） | 不倒回 |
| preset | 預設集設定 | app 沒有預設集：資料根目錄 `card-vars/preset.json`，固定身分 `app`〔模型判斷·未裁決〕 | 不倒回 |
| script | `script.data`（依 `script_id`） | `worlds/<id>/card-vars/script/<身分檔名>.json` | 不倒回 |
| extension | `extension_settings[extension_id]` | 資料根目錄 `card-vars/extension/<身分檔名>.json` | 不倒回 |

- 身分檔名＝原 ID 的 SHA-256 前 32 個十六進位字元；檔內存 `{ id: 原 ID, rev, vars }`，讀到時核對原 ID，不符當不存在。原 ID 任意字串照收（長度 ≤ 256）。
- 每檔自帶 `rev`；寫入帶目標 rev，不符回 `stale` 附新值（跨桌改 global 會這樣），宿主照被拒處理。app 同時只有一個卡片面板，提交後推給本面板即可；面板掛載時一律重讀。
- 根目錄的檔不在 `worlds/` 下，不能用 `commit_world_write`：新增根目錄版原子寫（暫存檔＋改名），同樣檢查更新閘門、用同檔鎖。

#### 8.8 上限（依真卡量測）
量測（2026-10-03，initvar 轉 JSON）：bcd368 22.5 KB、深度 6、357 葉、最長字串 150 字；DongeonMaster 3.7 KB、深度 8、133 葉。遊玩中會長，預留約百倍：
- 單次寫入整張表 ≤ 2 MB；深度 ≤ 32；單一字串 ≤ 64 KB；單一物件鍵數、單一陣列元素數 ≤ 10 000；節點總數 ≤ 200 000。
- 鍵：非空、≤ 256 字元；`__proto__`／`constructor`／`prototype` 存成自有屬性。
- 數值：任一處（含陣列元素）非有限數整批拒絕。
- 非 message 層每檔 ≤ 4 MB。
- 逐字稿每則事件都帶完整表會讓檔案變大：bcd368 一回合約多 25 KB，百回合約 2.5 MB，列已知限制。

#### 8.9 parseMessage（2c）
- 照 MVU `updateVariables` 重寫，不用 app 機制層解析器：指令 `_.set`、`_.insert`／`_.assign`、`_.remove`／`_.delete`／`_.unset`、`_.add` 與 JSONPatch（replace／add／insert／remove／move／delta）；括號配對抽取、`//` 理由；值解析依序 literal → JSON → JSON5 → 單引號 YAML → 受限數學式 → YAML → 字串；set 分支的 `Number()` 轉型；`[值, 說明]`；schema 規則；display／delta 字串。
- 事件分階段、可修改：流程在沙盒端跑、依序 await 監聽器——`VARIABLE_UPDATE_STARTED(variables)` → 抽指令 → `COMMAND_PARSED(variables, commands, message)`（監聽器可改 commands）→ 逐條套用（每條 `SINGLE_VARIABLE_UPDATED`）→ `VARIABLE_UPDATE_ENDED(variables, before)`；`_for_zod` 變體與 `BEFORE_MESSAGE_UPDATE` 照原始碼。這些事件的參數不做隔離拷貝。
- 值解析與數學式在宿主專用 Web Worker 跑（mathjs 受限實例、json5、js-yaml，D1）；沙盒逐條送字串、await 結果。單次計算 200 ms 上限，逾時 `terminate()` 該 Worker、回錯並重建。
- 不落檔、不記帳。上限：訊息 ≤ 256 KB、指令 ≤ 1000 條、單一數學式 ≤ 1000 字、結果同 8.8、整次 ≤ 5 秒。

#### 8.10 Sol 第 10 輪 8 項對照
- 活樓共用屬降級：不再適用——每樓各存一份（8.1）。
- 缺 `stat_data` 保留鍵不存在、區分尚無表與明確取代：8.1、8.3。
- 恢復衝突、state＝old／快照＝new：不再適用——沒有兩檔提交與意圖檔。
- 一致讀：8.2（來源＋版本）。
- `turn_id` 階段核對：8.3 GM 回合。
- 佇列按完整目標保存版本、不互相覆蓋或代結算：8.5。
- script／extension ID 改編碼映射檔名並保留原 ID：8.7。

Sol 第 12 輪 9 項：種子獨立存放與舊桌首次啟用（8.1）；版本改 token、整桌還原用桌世代（8.1）；逐樓 API 不繼承、初始化來源分開（8.2）；明確表原樣回傳、衍生值只在建表時產生（8.2）；模式交接保資料、不因停用腳本切回（8.1）；樹寫入者逐一改寫（8.4）；目標身分與讀值來源分開（8.2、8.3）；回合各階段核對 `turn_id` 與重複落檔冪等（8.3）；整桌還原以世代失效（8.1、8.4）。

Sol 第 13 輪 6 項：邊界改模式 epoch＋表所屬 epoch（8.1）；模式與種子合成單一控制檔、交接順序與重做（8.1）；新種子沿用完整表（8.1）；初始化來源限目前這一幕、各幕自己的種子（8.2、8.4）；匯入只改單一權威（8.4）；回合冪等鍵 `(turn_id, turn_part)`（8.3）。

Sol 第 14 輪 4 項：恢復誤刪的 8.3 後半與 8.4 開頭（對照 24f0eac）；epoch 改每幕各自一份（8.1）；幕操作的發布順序與失敗處理（8.4）；`main` 落檔即解除 busy、`turn_key` 寫進事件供重試辨識（8.3）。

#### 8.11 規模
估計包 2 合計約為包 1 的 5～6 倍（與上一版相同）：2a 約 3 倍、2b 約 1 倍、2c 約 2 倍。這輪新增的種子檔、模式交接、樹寫入者改寫與回合冪等落在 2a，抵掉逐樓繼承拿掉的部分，總量大致不變。

## 包 2 拍板

- 包 2 拆三包、同案依序做（C2）：2a 寫入核心（狀態提交鎖遷移、`state_rev`、兩檔提交、message 層讀寫、型別保留、歷史樓、宿主佇列）→ 2b 非 message 層（B2 六層）→ 2c parseMessage；每包各自送審與驗收，全部做完才合併 main〔作者裁決 2026-10-03〕。
- 卡片變數只存一處：逐字稿每則事件存完整 `message_vars`，卡寫只改目標事件、不改 state.json；桌面有效狀態（面板、GM 提示、apply_block、手改、新事件初始化）一律經同一投影入口，以最新一則帶變數的樓為準（同 MVU 以最新樓為下回合基準），state.json 樹降為可重建快取〔作者裁決 2026-10-03〕。已取代先前的兩檔提交／意圖檔恢復設計。
- parseMessage 數學式照做（D1）：新增 mathjs（宿主端、動態載入，不進 srcdoc）與 json5，值算法與酒館一致〔作者裁決 2026-10-03〕。

## 包 1 施工結果（2026-10-03）

- 施工待查四條：
  - GM／開場事件辨識：見第 1 節（`narration`＋`speaker_id` 空字串，含中止回覆）。
  - 索引：`pickCardShell` 先 `chatEvents` 排除 gm_only，再用同一份陣列算狀態來源、補占位、樓號（測試涵蓋 gm_only 夾在中間）。
  - 第 0 節：`npm run build` 產物含 jQuery 3.7.1 與 lodash 4.17.21。第 5 節：開場白不補占位，「开局」不被搶（單元測試）。第 6 節見下。
  - 非 message 層：包 1 期間 `Mvu.getMvuData` 的 `type` 不是 `message`（含沒給 option、沒給 type）一律拋錯「尚未支援 <type> 層變數」，不回空物件、不改讀 message 層；`getVariables` 等酒館助手變數函式包 1 不定義。包 2 照 B2 補齊後取代。
- 第 6 節靜態對照（實際提示待 GUI 驗收抓）：兩卡都走 `CardFormat`（有顯示腳本、沒骨架）。bcd368 的格式條目會被認成「前端」（含 `<BreederApp>`），DongeonMaster 認不到條目。導演指示寫「不要輸出規定格式以外的任何說明或狀態欄」，沒提 `<UpdateVariable>`；`[mvu_update]` 輸出格式條目匯入時已收編停用，更新規定只剩增量桌的協定聲明。有壓掉變數更新的風險，要真模型一回合才判得準，排進實測佇列；確認衝突就另案，本案不宣稱後續回合可用。
- 真卡整合測試：`card-mvu-shim.test.ts` 讀 `TestCards/` 的 bcd368 兩支 MVU 前端與 DongeonMaster「迷宫之书」，在 jsdom 跑整份 srcdoc：首次渲染有值、推送後重畫。TestCards 不在 repo，CI 上略過。

## 包 2a 施工結果（2026-10-03）

- **上游核對**：照 MagVarUpdate `438f9ffc`、JS-Slash-Runner `46ec10df` 逐行核〔Sol〕條目「寫入 `'latest'` 取最後一樓含 system」：酒館助手 `replaceVariables` 把不給／`'latest'` 正規化成 -1 後 `chat.at(-1)`（含 system），讀取的 `'latest'` 才先濾掉 system，與計畫一致。順帶核 8.6 兩表（insert 既有值優先、insertOrAssign 陣列整個取代、`delete_occurred`、updateVariablesWith 讀用讀取樓號／寫用寫入樓號；MVU `setMvuVariable` 路徑不存在回 false、任何長度二陣列改第 0 項、display／delta 只走 `stat_data.$internal`），一致。SillyTavern 沒有〔Sol〕條目要核。
- **效能閘**（M 系列 Mac、release 版；`message_vars/tests.rs` 的 `perf_gate_…`，`--ignored` 實跑）：500 則、每則 25 KB 表＋同大小狀態快照（逐字稿 20.6 MB）——整幕完整解析 82–96 ms（大頭是既有的逐則狀態快照；表以原始 JSON 存，只掃不建）、卡寫 54–64 ms（鎖內單行讀改＋原子替換＋快取重建）、`read_state` 投影 13 ms。只帶表不帶快照時讀＋整檔改寫約 35 ms。
- **落點**：後端 `data/state_commit.rs`（短提交鎖）、`data/message_vars/`（json 保留鍵順序與上限、convert 樹↔表、control 控制檔、source 初始化來源與投影、mode 交接與快取、turn 世代與回合紀錄、write 寫入路徑）、`data/scene/transcript/lines.rs`（單行讀改）、`commands/card_vars.rs`（`card_vars_state`、`card_vars_write`、`set_player_card`）。前端 MVU 檔案搬進 `features/card-interface/mvu/`：`card-mvu-shim.ts`（快照）、`card-mvu-shim-source.ts`（沙盒原始碼）、`card-mvu-write.ts`（宿主佇列與上限）。自製測試卡 `scripts/harness-fixtures/mvu-write-probe.json`。
- 施工取捨〔模型判斷·未裁決〕：
  - 表以原始 JSON 文字存在事件上，讀改用保留鍵順序的 JSON（卡片照 `Object.entries` 順序畫清單）。
  - 卡寫、改初始化來源、回合冪等查找只解析命中的那一行、其他行原樣拼回（為了效能閘）。
  - 換幕時新幕開頭的摘要事件帶一份種子表（vars_epoch＝新幕 epoch），第一個 GM 回覆前卡片照樣讀得到值、補得到占位〔Sol 2a 驗收第 1 輪要求維持殼選路〕。
  - 資格＝有啟用中的 MVU 卡（換幕結算自動隱藏的卡不算，與前端卡片介面同一份清單）、沒有非空重構骨架、不是 characters 桌。
  - 控制檔另存 `macros`（啟用當下的 `{{user}}`／`{{char}}`），之後匯入補值照它代換。
  - 變數模式下這一幕沒有種子（例如退回啟用前的父幕）：當作還沒啟用，下一次開場／GM／卡寫以當下樹物化這一幕的種子。
  - 分岔只把來源幕「有效 epoch」的表改成新幕 epoch，舊 epoch 的表維持無效（計畫寫全部改）。
  - 換幕 ② 前先刪掉新幕逐字稿的殘留，重試不會疊兩則摘要。
  - 卡寫只收目前這一幕的事件，別幕回 stale。
  - 面板手改的 busy 只在變數模式擋，樹模式維持現狀；新增 UiMsg `state_edit_during_turn`。
  - `chat_abort` 不動回合紀錄：中止由 `gm_narrate` 的結果決定（完成與取消同時就緒取完成），出錯／空回覆時回合守門把紀錄改成已中止。
  - 回合交接：提交時把正文與 `state_update` 變動紀錄在同一次鎖內一起登記進回合紀錄的待落清單，中止留下的半截正文（`truncated`）也記進去。前端沒落成時由後端依序用同一個冪等鍵代落（正文掛表，變動紀錄與半截不掛），時機是：任何不屬於進行中回合的新事件追加前（`append_transcript`／`append_event` 不帶鍵／開場／換幕摘要都走同一個鎖內入口，繞不過）、貼開場存檢查點前與 `append_opening` 鎖內最前面、換幕／分岔／退幕前、下一個 GM／角色回合開始時。同回合內部的附屬追加（GM 提交後的登場紀錄）要拿 `begin_turn` 發的 `TurnTicket` 走 `append_within_turn` 才不交接。任一則代落失敗，新事件就不追加；生成中的回合擋換幕、不能被新回合蓋掉；`main` 已落檔卻被收回時，晚到的重試回錯、不復活。回覆還沒落檔時不收回觸發它的玩家句（前端改放失敗彈窗）。正常回覆被供應商截斷時，由該次 GM 呼叫的回傳（`stream_turn_reporting_truncation`）帶回標記，代落的正文照樣帶 `truncated`；不存跨呼叫的共用狀態，同桌並行的翻譯／摘要碰不到。`append_opening` 在鎖內最前面先交接，開場表以代落後的來源為底。
  - GM 沒有任何狀態更新時照樣提交：變數模式下 main 掛來源表的拷貝（照 MVU 每則 AI 樓都有表）。
  - 開桌（`open_world`）一律算整桌交換：世代加一。
  - 前端：還沒啟用（樹模式）時照包 1 由狀態樹推每樓資料，卡寫會觸發啟用；空桌時代替開場白的那一樓讀目前狀態、不能寫。
  - 沙盒本地值依寫入目標暫存，快照追上確認的版本才丟（確認早於畫面更新時不閃回舊值）；`replaceVariables` 等同步 API 被拒只記 `console.error`；非有限數、循環參照在沙盒端就整批拒絕（JSON 會把非有限數悄悄變 null）。
  - 復原（`append_transcript` 帶回整則事件）照收前端給的表、只換新版本 token，不另驗上限（來源是 app 自己的前端）；前端把表以 JSON 文字送回，保住鍵順序。
  - 短提交鎖可重入（同執行緒已持有就不重取）；`read_state` 投影在鎖內讀；state.json 的讀改寫一律走 `update_state`（鎖內）。
  - 逐字稿改寫（卡寫、改來源表、收回、改快照、改摘要）與 state.json 一律暫存檔＋改名的原子替換（寫到一半失敗原檔不動）。
  - 沙盒寫入帶產生那張表時的底版、桌世代與幕；宿主同目標寫入鏈的第一筆以底版當預期版本，之後換成每次確認的新版本；推來的快照只有是「本地值之前的舊版本」才照留本地值。
  - 舊事件（`@位置`）第一次寫入配到 id 後，宿主佇列把整條寫入鏈（含在飛期間排著的）搬到新 id、之後送來的 `@位置` 寫入也改寫到 id；結算訊息帶 `migrate`，沙盒把本地值、在途筆數、版本紀錄與快照的寫入目標一起搬過去。
  - 殼 key 含桌世代與幕：整桌還原（同 id 事件帶回）或換幕就重掛、舊佇列關閉（未結算 reject）。卡寫 await 回來先重問桌世代與幕，問完再核一次前端目前的桌、世代與幕，任一不同就回 `gone`：不換進逐字稿、不推權威值，在飛與未送的一起 stale。換進逐字稿的那一刻再同步核一次身分；佇列關掉後晚到的成功結果，身分沒變才換（逐字稿與磁碟一致），變了就丟。
  - GM 提交時狀態快取（樹模式下是權威）沒寫成：回 `state_error` 給前端提示，變數表留在回合紀錄照樣隨正文落檔；前端正文落檔失敗以同鍵重試三次。
  - 換幕、分岔、退幕在 GM 回合生成中擋下（新增 UiMsg `scene_change_during_turn`），正文未落檔時先代落再動幕；退幕先寫 current_scene 切回父幕，成功後才刪子幕逐字稿與種子。

## 已知限制

- 變數模式桌的逐字稿每則 GM／開場事件帶完整變數表，檔案較大（8.8）；每則事件另帶的狀態快照（含樹）讓整幕完整解析接近 100 ms（效能閘數字），慢機器可能翻倍。
- MVU 卡在換幕結算被自動隱藏時不算符合資格（卡片介面同樣不出現），這時重構復原不會自動啟用變數模式。
- 重構復原時種子由當下狀態樹重新物化：接管模式下改過、且與原表不同的值，型別照包 1 規則推斷（例如原本是字串 `"123"` 被改過就會變成數字）。

- 真卡整合測試的侷限：狀態樹是手造的、module script 改成一般 script、在 jsdom 不在 WebView；真實 initvar 匯入後的樹、按鈕互動、module 與 WebView 行為留 GUI 驗收。
- 舊紀錄的開場白事件沒有 `opening` 標記，會被當成 AI 回覆補占位（發佈前舊桌不相容可接受）。

- 改過的葉子若含玩家名，存成字面名字，不換回 `{{user}}`。

- `display_data` 不含「舊值->新值」、`delta_data` 為空：app 只存變動標記不存舊值。
- `getAllVariables` 只有 MVU 這層，沒有酒館的全域／角色／聊天變數（包 2 照 B2 補）。
- 被引號包住的數字字串（`"123"`）匯入時引號已剝，會還原成 number；前導零數字保留字串（與 YAML 不同）。
- YAML 行內陣列沒寫成合法 JSON 的保留字串。
- 推送值沒變時不發事件（上游會發）；`VARIABLE_INITIALIZED` 不發；`tavern_events` 等其他事件不觸發。

## 測試清單

- **`card-mvu-shim.test.ts`**：
  - 型別還原：整數、負數、小數、指數（`1e3`、`-2.5E-2`）、邊界（`007`、`.5`、`1.`、`+1`、`1e999` 溢位、`-0`）、布林、null、`[]`、`{}`、`[1, "說明"]`、壞 JSON、YAML 行內陣列、`#` 開頭字串。
  - JSON 巢狀非有限數字整個保留原字串：陣列裡（`[1, [2, 1e999]]`）與物件裡（`{"a": {"b": -1e999}}`）。
  - value_types 的 number／bool／list 優先，number 型非法值保留字串。
  - 結果不含 NaN／Infinity，深拷貝後形狀不變。
  - `$meta` 保留；`{{user}}` 遞迴代換（巢狀、陣列、`$meta.template`）與鍵撞名。
- **來源樓**：
  - 本樓有快照；往前找。
  - 最新 GM 殼後面接玩家句（玩家句也帶快照）：GM 殼仍算活的，手改值照樣收到。
  - 最新 GM 殼後面接角色台詞、系統事件：同上。
  - 先手改值、再送玩家句（玩家句快照是改後值、GM 事件快照是改前值）：GM 殼顯示改後值。
  - 歷史殼（後面還有 GM 回覆）不讀未來樓與手改值，即使中間夾著玩家句。
  - `getMvuData` 指定歷史樓與活樓各回對的資料。
  - 空桌、只有開場。
- **墊片**：
  - `getAllVariables` 深拷貝隔離；`waitGlobalInitialized('Mvu')` 立即 resolve，未知名稱持續等待。
  - `getMvuData` 的正數、負數、`'latest'`、不給 `message_id`、不存在樓號；尾樓是 system 時 `'latest'`／預設取前一則非 system 樓、`-1` 取 system 尾樓；`getMvuVariable` 三類別、`[值, 說明]` 取第一項、預設值。
  - 推送驗來源、token、形狀；值相同不發事件；值不同時 STARTED 只帶舊資料、SINGLE 逐葉、ENDED 帶（新, 舊）且順序正確。
  - `eventOnce` 只觸發一次；`eventRemoveListener`；監聽器拋錯不影響其他監聽器；監聽器改參數不污染快照。
- **`card-sandbox-libs.test.ts`**：
  - `$`、`_` 可用；重載庫後仍可用。
  - `errorCatched` 保留 this、參數、回傳值，同步例外與 Promise rejection 都記錄。
  - 內嵌文字不含未跳脫的 `</script>`。
- **整合（jsdom 跑完整 srcdoc）**：
  - bcd368 兩支 MVU 前端與 DongeonMaster `迷宫之书` 三支實際腳本首次渲染出值；推送後重畫；按鈕送出仍走誘餌。
  - 真實匯入後的樹（`$meta`、`[]`）轉出的 stat_data 結構正確。
  - 墊片順序（庫 → 讀訊息 → MVU → 橋接 → 卡片）。
- **占位**：
  - 符合條件才補、不重複補；選殼與 getChatMessages 用同一份文字。
  - DongeonMaster 開場「开局」不被搶走。
- **controller／overlay**：
  - 手動改值不重掛只推送；換樓重掛帶新值。
  - load 重推去重；快速切桌、收回、關掉重開面板不串值。
- **包 2（寫入）**：
  - 資料模型：新事件有 ID；舊事件以場別＋序號＋ts＋內容指紋定位、第一次卡寫補 ID；同 ts 兩則不混淆。`message_vars` 往返保留型別、缺鍵、`display_data`／`delta_data`；尚無表與「明確取代成沒有 stat_data」讀起來不同。
  - 讀取：逐樓 API 讀無表的樓回 `{}`、明確表原樣回傳（缺 display／delta 就是缺）；`getAllVariables` 會帶出前樓值；初始化來源只看這一幕、目前 epoch，依位置取最新帶表的樓（改寫較早的樓不會讓它變來源）；收光帶表事件回到這一幕的種子而非快取；換幕、退幕、分岔不讀到已離開的幕；`read_state` 的 `events` 模式回投影、`tree` 模式逐字不變；快取寫失敗不影響提交、下次重建。
  - 模式交接：舊桌首次啟用物化當下值、觸發啟用的尾樓卡寫算數；重構→改值→復原後讀到重構期間改的值、`schema` 等鍵保留；停用腳本不切回；`events` → `tree` 在寫樹後、寫控制檔前崩潰，重做後結果正確；控制檔寫失敗模式不變。
  - 幕：換幕的新幕種子、退幕移除種子、分岔重配 id 與 epoch；子幕重構往返後退回父幕，父幕的表仍有效；換幕、分岔每一步後失敗再重試，不留「目前幕沒有種子」、不覆寫已發布的幕；退幕切回失敗時子幕種子不動。
  - 匯入初值與匯入復原只改初始化來源那一份。
  - 版本：同一則移除最高版本再復原不撞號；整桌還原（帶回相同 id 與 token）後舊請求因世代被拒。
  - 卡寫：目標版本相符才寫、不符回 `stale` 附權威值；寫歷史樓只影響那一樓；寫 `'latest'` 取最後一樓含 system（讀取的對照測試）；寫入沒有表的 B 只動 B、不動 A。
  - 效能：500 則、每則帶 25 KB 表的逐字稿，讀取與整檔改寫時間。
  - GM 回合：生成中卡寫與手改回 `busy`；提交後到前端落檔前也擋；提交、落檔、中止、錯誤、換桌清除都核對 `turn_id`，不符不改狀態；同 `(turn_id, turn_part)` 重複落檔回原事件不重追加、`main` 與 `state_update` 各自冪等；寫檔成功但回傳失敗的重試認得出已提交；`main` 落檔後立即解除 busy；`state_update` 永遠不掛表；落檔失敗重試；中止保留半截正文落成不帶表的事件；GM 沒改的路徑保留原型別；前端正文三次都沒落成時，下一次玩家送出前依序代落正文與變動紀錄（中止半截同樣），且不收回那句玩家發言；state.json 寫到一半失敗（GM 提交、換幕、分岔、退幕發布）原檔不動。
  - 手改改來源事件；收回移除事件變數、復原帶回並換新版本，收回再復原後舊版本的在途寫入被拒（ABA）。
  - 整桌交換／還原（備份還原、格式轉換、匯入復原、重構套用與重置、`OpeningCheckpoint::restore`）後，回合階段清空、在途卡寫因目標不存在被拒。
  - `append_opening` 走 tx 版不自鎖；`set_player_card` 只改那一欄。
  - 沙盒 API：8.6 表逐項（insert 既有值優先、insertOrAssign 陣列整個取代、`delete_occurred` 缺路徑情形、預設層 chat）；`setMvuVariable` 不送宿主、任何長度二陣列改第 0 項、不轉 Number、`is_recursive` 才發 SINGLE；整表 replace 缺鍵語意。
  - 宿主佇列：同目標 W1 在飛時 W2、W3 合併；不同目標（兩個樓、兩個層）各自結算、互不覆蓋；W1 成功 W2 被拒（重讀權威值後推回、未送版本一起 reject）；換殼／切桌／關面板時所有未結算 Promise 都 reject；每個 requestId 都有結算；舊事件連寫第二筆搬到新 id；await 後桌世代變了的結果不換進逐字稿。
  - 上限各項（含陣列元素非有限數）。
  - 非 message 層（2b）：六層落檔與固定身分、身分檔名雜湊與原 ID 核對、根目錄原子寫與更新閘門、跨桌 rev 衝突、收回不倒回、`getAllVariables` 合併順序。
  - parseMessage（2c）：各指令與 JSONPatch、值解析六段、set 分支 Number 轉型、schema 規則、`[值, 說明]`；STARTED／COMMAND_PARSED／SINGLE 監聽器改資料會影響後續；事件順序與參數；Worker 逾時被終止並重建；不落檔；各上限。
- **寫後重掛**：寫入成功 → 關掉重開面板 → 讀回寫入值與型別；收回上一句後 message 層跟著倒回、其他層不倒回。

## 驗收（GUI，零額度）

test-harness 獨立 root，不啟動正式包：
1. 匯入 bcd368，空桌打開面板：用户数据／便器数据库顯示 initvar 的值。
2. `post_opening` 寫兩則帶 `<UpdateVariable>` 的假 GM 訊息改值（例如资金、开发阶段），一則不帶占位、看 app 補占位：面板顯示第二則的值；收回 → 第一則的值。
3. 面板開著、尾樓是玩家句時，在狀態欄手動改一欄：不重掛、面板自動換值。
4. DongeonMaster：開場「开局」畫面正常；同 2、3（基础信息.时间、迷宫房间）。
5. 第 6 節的 GM 提示對照（門檻）。
6. 正式 build 產物確認含兩個庫。
6a. 包 2 寫入（兩張受惠卡都不寫入，改用自製測試卡）：
   - 測試卡 `mvu-write-probe`：placement 2 的 regex 命中 `<StatusPlaceHolderImpl/>`；殼裡顯示 message 層幾個欄位（含 `[值, 說明]`、`{{user}}`、字串 `"123"`）與 chat／global 層各一欄；按鈕逐一呼叫 8.1 表的寫入函式、寫歷史樓、寫超大值、parseMessage 預覽。initvar 用停用的 `[initvar]` 條目，帶載入 MVU 的 tavern_helper 腳本。
   - 放 repo 內 test-harness 的 fixture（不放 gitignore 的 TestCards），驗收能重跑。
   - 驗：寫入後狀態欄與目標事件的變數都變；關掉重開面板值與型別還在；收回上一句 message 層倒回、chat／global 不倒回；開另一桌看得到 global 值；歷史樓只改那一樓；超大值被拒、面板回到權威值；`{{user}}` 欄位不被改寫。
7. ai-log 零派送；quit 後兩個正式目錄 hash 不變。
