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
- 寫入（第 8 節）：
  - 新增 `src/features/card-interface/card-mvu-write.ts`（＋test）：宿主端驗證、反向轉換、diff、寫入佇列。資料夾 → 17 檔。
  - `useCardInterfaceController.ts` 收 `mvu-write`／`mvu-parse` 訊息；`App.tsx` 傳回合忙碌旗標與 `tableState.refresh`。
  - 後端：`commands/state.rs` 新增 `write_card_variables` 與 `preview_card_update`；樹操作放 `mechanism/tree.rs`（或 `data` 層既有的樹函式旁），commands 只做邊界。

### 8. 寫入（A3）

#### 8.1 上游寫入 API 與 app 對應
上游規格（2026-10-03 讀 MagVarUpdate `src/function/global/index.ts`、`update_variables.ts` 與酒館助手 `@types/function/variables.d.ts`）。酒館助手這組函式的 `option` 都是 VariableOption；本節只講 `type: 'message'`，其他層見待拍板 B。

| API | 上游語意 | app 行為 |
|---|---|---|
| `getVariables(option)` | 回該層變數表；message 樓號預設 `'latest'`、負數從尾算、超出範圍拋錯 | 回該樓 `{stat_data, display_data, delta_data}` 深拷貝 |
| `replaceVariables(vars, option)` | 同步、回 void；整張表換掉 | 沙盒本地立即換掉；`vars.stat_data` 送宿主寫回 |
| `updateVariablesWith(updater, option)` | updater 拿到目前表回新表，可回 Promise；回新表（或其 Promise） | 同上，Promise 版在 resolve 後才寫 |
| `insertOrAssignVariables(vars, option)` | 深合併：新增或覆寫；回新表 | 合併語意（陣列整個取代還是逐項合併）施工時對照上游實作定，寫回本檔 |
| `insertVariables(vars, option)` | 只補不存在的鍵；回新表 | 同上 |
| `deleteVariable(path, option)` | 刪路徑；回 `{variables, delete_occurred}` | 同上；`delete_occurred` 照本地結果 |
| `Mvu.getMvuData(option)` | 等於 `getVariables` | 同第 3 節 |
| `Mvu.replaceMvuData(data, option)` | 等於 `replaceVariables`，回 Promise | 同 `replaceVariables`；Promise 在宿主確認落檔後 resolve，被拒時 reject |
| `Mvu.setMvuVariable(data, path, value, {reason, is_recursive})` | 只改傳進來的 `data.stat_data`，**不落檔**；路徑存在且改成功回 true；舊值是 `[值, 說明]` 時只改第 0 項；舊值是數字時把字串 `Number()`；display／delta 寫 `舊->新 (reason)`；發 `SINGLE_VARIABLE_UPDATED`（觸發條件與 `is_recursive` 的關係施工時核對） | 照做，純沙盒內運算，不送宿主；要落檔由卡片接著呼叫 `replaceMvuData`（上游用法） |
| `Mvu.parseMessage(message, old_data)` | 複製 old_data、套用訊息裡的更新指令、回新資料，不落檔 | 送宿主 → 後端 `preview_card_update` 用現有 `mechanism::parse_updates`＋套用邏輯算在複本上，不落檔、不記帳；回 Promise |

- 上游的 `replaceVariables` 等不發 MVU 事件（MVU 事件只在處理 AI 回覆時發）。app 照做：卡片自己寫入不發 STARTED／ENDED。
- 寫入目標樓解析和讀取一樣；目標樓的來源是「活的那一份」（第 1 節：最後帶狀態事件及之後的樓）才准寫。

#### 8.2 歷史樓寫入
照上游語意允許改該樓快照、不覆寫目前狀態〔作者裁決 2026-10-03〕；細節包 2 重寫。

#### 8.3 寫入流程
1. 沙盒：寫入函式先改本地 store（保持上游同步語意：寫完馬上 `getVariables` 讀得到），再送 `{source:"table-tavern-card", kind:"mvu-write", token, requestId, floor, stat_data}`。只送 `stat_data`；`display_data`／`delta_data` 是衍生資料不落檔；其他頂層鍵見已知限制。
2. 宿主 controller 驗訊息：只收目前 iframe 的 contentWindow（`event.source` 比對）、token 等於目前殼、面板開著、目標樓仍是活的。
3. 宿主把新 stat_data 與它上次推給沙盒的那份比對，算出 diff（只含變動葉子的 set／delete）。只寫 diff 的理由：沒變的葉子保留原字面（例如 `{{user}}`）；代換過的鍵用讀取時留下的對照表換回原字面路徑。
4. 反向轉換：物件→分支（`{}`→空分支）；陣列→合法 JSON 字串葉子（`[值, 說明]` 照存，讀回來還原成陣列）；數字→`String()`；布林→`"true"`／`"false"`；null→`"null"`；字串原樣。不套 value_types 檢查：上游 `replaceVariables` 也不驗；型別不符的值存成字串，讀取時照第 2 節規則還原。
5. 呼叫後端 `write_card_variables(world_id, base, ops)`：
   - `base`＝宿主算 diff 時「最後一則帶狀態事件」的 ts 與事件總數。
   - 後端拿 `world_write_permit_async`（與回合、`set_state_path` 同一把世界寫入鎖），鎖內重讀狀態與逐字稿，比對 base；不符（中間有新回合落檔、收回等）就整批拒絕。
   - 套 ops 到 `state.tree`，寫 `state.json`，再 `set_last_transcript_state` 同步最後一則快照（與 `set_state_path` 同一路徑）。一批全成或全不寫。
   - 新增 `delete` 與 `set` 兩種 op：`set` 寫空字串是存空字串，不沿用 `set_state_path`「空值＝刪除」的語意。
   - 不走 mechanism 的欄位規則（唯讀、夾限、骰值）：上游前端寫入也不經 MVU 的模型更新檢查。寫入記一筆 mechanism-log（kind 新增 `card_write`）留痕。
6. 成功：宿主 `tableState.refresh()` → 活狀態更新 → 推送。沙盒本地已是同值，推送比對相同、不發事件。`replaceMvuData` 的 Promise resolve。
7. 失敗：宿主照常推送權威快照，沙盒換回並發 `VARIABLE_UPDATE_STARTED`／`ENDED`，讓卡片重畫成真值〔模型判斷·未裁決：上游沒有拒絕這回事〕；`console.error` 原因；`replaceMvuData` 的 Promise reject。

#### 8.4 信任邊界（宿主端與後端都驗，後端為準）
- 訊息形狀：`requestId` 字串、`floor` 整數、`stat_data` 純物件（非陣列、非 null）。
- 大小：序列化後 ≤ 1 MB；深度 ≤ 32；單一字串葉子 ≤ 64 KB；ops ≤ 5000；寫入後整棵樹序列化 ≤ 4 MB。
- 鍵：非空字串、≤ 256 字元；拒 `__proto__`、`constructor`、`prototype`。
- 值：數字必須有限；不收函式、undefined（JSON 往返後也不會出現，宿主仍檢查）。
- 任一條不符：整批拒絕，不部分寫入。
- 頻率與並發：
  - 宿主一桌一條寫入佇列，同時只有一個請求在飛。在飛期間的新寫入合併成「最新一份 stat_data」，回來後再對最新確認值算一次 diff 送出。卡片連點不會灌爆後端，最後值不會丟。
  - 回合進行中（前端忙碌旗標為真）：宿主直接拒絕並推回權威值，不讓請求排在回合後面。回合落檔後 base 也會不符，後端仍會擋。
  - 切桌、換殼（token 變）、面板關閉：佇列丟棄未送出的寫入，在飛的那筆回來時 token 不符就不推送。

#### 8.5 與事件、`[值, 說明]`、value_types 的互動
- 卡片自己寫入不發事件；外部變動（GM 回覆、手動改值、寫入被拒）照第 4 節發事件。
- `setMvuVariable` 遇到 `[值, 說明]` 只改第 0 項；落檔時整個陣列存成 JSON 字串，讀回來仍是陣列，說明不會丟。
- value_types 是 number 的欄位被寫成非數字字串：照存；讀取時第 2 節規則保留字串；GM 下一輪套更新時由 mechanism 原有規則處理（字串加減記硬錯誤），不在本案改。


## 包 1 施工結果（2026-10-03）

- 施工待查四條：
  - GM／開場事件辨識：見第 1 節（`narration`＋`speaker_id` 空字串，含中止回覆）。
  - 索引：`pickCardShell` 先 `chatEvents` 排除 gm_only，再用同一份陣列算狀態來源、補占位、樓號（測試涵蓋 gm_only 夾在中間）。
  - 第 0 節：`npm run build` 產物含 jQuery 3.7.1 與 lodash 4.17.21。第 5 節：開場白不補占位，「开局」不被搶（單元測試）。第 6 節見下。
  - 非 message 層：包 1 期間 `Mvu.getMvuData` 的 `type` 不是 `message`（含沒給 option、沒給 type）一律拋錯「尚未支援 <type> 層變數」，不回空物件、不改讀 message 層；`getVariables` 等酒館助手變數函式包 1 不定義。包 2 照 B2 補齊後取代。
- 第 6 節靜態對照（實際提示待 GUI 驗收抓）：兩卡都走 `CardFormat`（有顯示腳本、沒骨架）。bcd368 的格式條目會被認成「前端」（含 `<BreederApp>`），DongeonMaster 認不到條目。導演指示寫「不要輸出規定格式以外的任何說明或狀態欄」，沒提 `<UpdateVariable>`；`[mvu_update]` 輸出格式條目匯入時已收編停用，更新規定只剩增量桌的協定聲明。有壓掉變數更新的風險，要真模型一回合才判得準，排進實測佇列；確認衝突就另案，本案不宣稱後續回合可用。
- 真卡整合測試：`card-mvu-shim.test.ts` 讀 `TestCards/` 的 bcd368 兩支 MVU 前端與 DongeonMaster「迷宫之书」，在 jsdom 跑整份 srcdoc：首次渲染有值、推送後重畫。TestCards 不在 repo，CI 上略過。

## 已知限制

- 真卡整合測試的侷限：狀態樹是手造的、module script 改成一般 script、在 jsdom 不在 WebView；真實 initvar 匯入後的樹、按鈕互動、module 與 WebView 行為留 GUI 驗收。
- 舊紀錄的開場白事件沒有 `opening` 標記，會被當成 AI 回覆補占位（發佈前舊桌不相容可接受）。

- 歷史樓變數唯讀（8.2）〔模型判斷·未裁決〕。
- message 層只落 `stat_data`；卡片寫進同一樓變數表的其他頂層鍵只留在沙盒本地，重掛就消失，寫入時 `console.warn`〔模型判斷·未裁決〕。
- 改過的葉子若含玩家名，存成字面名字，不換回 `{{user}}`。
- `Mvu.parseMessage` 只認 app 機制層支援的更新格式（`<UpdateVariable>`／JSONPatch 那套）；舊式 `_.set(...)` 指令若 app 不認，回 `undefined`（施工時確認 app 解析器支援範圍，寫回本檔）。

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
- **寫入（沙盒，`card-mvu-shim.test.ts`）**：
  - 八支寫入／讀取 API 的語意表逐項：寫完立即讀得到；`insertVariables` 不覆寫既有；`deleteVariable` 的 `delete_occurred`；`updateVariablesWith` 同步與 Promise 版。
  - `setMvuVariable` 只改傳入物件不送宿主、`[值, 說明]` 只改第 0 項、數字欄字串轉數字、路徑不存在回 false、display／delta 字串。
  - 歷史樓寫入同步拋錯且不送訊息；超出範圍樓號拋錯；非 message 層照 B 的裁決。
  - `replaceMvuData` 的 Promise 在確認後 resolve、被拒時 reject；被拒後 store 換回權威值並發 STARTED／ENDED。
- **寫入（宿主，`card-mvu-write.test.ts`）**：
  - 反向轉換：物件、空物件、陣列、`[值, 說明]`、數字、布林、null、空字串。
  - diff 只含變動葉子；`{{user}}` 原字面保留；代換過的鍵換回原路徑。
  - 驗證拒絕：非物件、超大（1 MB／64 KB 葉子／深度 33／ops 5001）、壞鍵（空、超長、`__proto__`）、非有限數字、錯 token、別的 source、面板關著、目標樓非活的、回合忙碌。
  - 佇列：在飛時連續寫入合併成一筆、最後值不丟；切桌與換殼丟棄。
- **寫入（後端，cargo）**：
  - `write_card_variables` 合法 set／delete 落 `state.json` 與最後一則快照；`set` 空字串存空字串。
  - base 不符整批拒絕且檔案不變；任一 op 非法整批不寫。
  - 與回合並發：回合持鎖期間送出的寫入等鎖後因 base 不符被拒，回合結果不被覆蓋。
  - 記帳 `card_write`。
  - `preview_card_update` 不落檔、不記帳。
- **寫後重掛**：寫入成功 → 關掉重開面板（重掛）→ `getAllVariables` 讀回寫入值；收回上一句後寫入值跟著倒回。

## 驗收（GUI，零額度）

test-harness 獨立 root，不啟動正式包：
1. 匯入 bcd368，空桌打開面板：用户数据／便器数据库顯示 initvar 的值。
2. `post_opening` 寫兩則帶 `<UpdateVariable>` 的假 GM 訊息改值（例如资金、开发阶段），一則不帶占位、看 app 補占位：面板顯示第二則的值；收回 → 第一則的值。
3. 面板開著、尾樓是玩家句時，在狀態欄手動改一欄：不重掛、面板自動換值。
4. DongeonMaster：開場「开局」畫面正常；同 2、3（基础信息.时间、迷宫房间）。
5. 第 6 節的 GM 提示對照（門檻）。
6. 正式 build 產物確認含兩個庫。
6a. 寫入（兩張受惠卡都不寫入，改用自製測試卡）：
   - 測試卡 `mvu-write-probe`：一支 placement 2 的 regex 命中 `<StatusPlaceHolderImpl/>`，殼裡顯示幾個欄位（含一個 `[值, 說明]` 與一個 `{{user}}` 欄位），按鈕分別呼叫 `replaceMvuData`、`setMvuVariable`＋`replaceMvuData`、`insertOrAssignVariables`、`deleteVariable`、寫歷史樓、寫超大值。initvar 用停用的 `[initvar]` 條目，帶 tavern_helper 的 MVU 腳本宣告。
   - 放 repo 內 test-harness 的 fixture（施工時看既有 harness 卡放哪；不放 gitignore 的 TestCards，驗收要能重跑）。
   - 驗：按鈕寫入後狀態欄與 `state.json` 都變；關掉重開面板值還在；收回上一句倒回；歷史樓與超大值被拒、面板顯示權威值；`{{user}}` 欄位不被改寫。
7. ai-log 零派送；quit 後兩個正式目錄 hash 不變。
