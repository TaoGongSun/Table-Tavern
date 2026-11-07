# room-open-from-idea：開房窗接「一句話生成世界」（方案）

來源：桌面版房主回饋「需要自選或原創世界觀，開房測試時世界觀是隨機的」。查證：開房窗只有「選角色卡開房…」「不選卡，直接開房」；不選卡＝空桌、`world.md` 空字串、GM 提示詞沒有世界設定段，GM 每次即興——這就是「隨機」。大廳「一句話開桌」（genesis）已能生成世界設定、角色卡、開場旁白，但開房窗沒接上；「自選」的「用這張桌開房」只在桌卡選單。作者選「照建議做」〔作者裁決 2026-11-07〕。

角色：作者指出很多玩家想自己建角色而不是挑生好的，就算能事後改也不會想到；手機入座畫面已有「建立角色」「匯入角色卡」但排在名單下面，名單空時它就是唯一選項。定案：開房版二選一、預設「朋友自己建角色」〔作者裁決 2026-11-07〕。全篇取捨未標來源者為〔模型判斷·未裁決〕。

案別：小案偏中，核心估 80–130 行（桌面版前端＋genesis 提示詞分流）。不做方案審，做完 Sol 驗收一次、Sonnet 截圖主線親看。

## 0 現況已有的（不重做）

- 一句話開桌：`GenerateTableDialog`（`useTableGenesis("chat")`）→ `generate_table_outline` → 大綱可改、AI 再加一位 → `generate_table_expand` → `genesis::materialize` 建新桌（`world.md`、每角色一張卡含公開／私密人設、開場旁白在第 0 回合）。提示詞把角色寫成「玩家會互動的 NPC，不要建代表玩家的角色」，對象是單一玩家。
- 房間名冊：`room/seed.rs` 以 `playable: !card.stage` 決定可不可入座，genesis 生的角色卡全部可入座（被當預製角色用）。手機 `SeatPicker`：名單在上，`allowCustomCards`（預設開）時下方有「匯入角色卡」「建立角色」（`seat.create` → 執行端建卡後自動入座）。
- 開房＝一定建新桌〔作者裁決 2026-11-03〕。`useRoomFlows.open` 的 existing 分支（桌卡「用這張桌開房」）：進桌 → `setPending` → 全部對話框收了才 `bind(id, server, fresh=false)`，桌名不改。
- `GenerateTableDialog` 在 `AppDialogs` 常駐一份，`open` 只控畫不畫、草稿留在元件裡；生成中不能關、點遮罩不關。

## 1 開房窗（`OpenRoomDialog.tsx`）

- 新桌版本三顆鈕，順序：「選角色卡開房…」（primary，不動）、「一句話生成世界開房…」、「不選卡，讓 GM 即興」（只改名，行為不變，讓房主按之前就知道世界會是 GM 自己編的）。
- 「一句話生成世界開房…」→ `onGenerate(server)`：關開房窗、開一句話開桌對話窗（開房模式）。
- 用現有桌開房（`existingTable`）的版本不動。

## 2 一句話對話窗的開房模式

- 題材列下方一個二選一（segmented 或 radio，開房模式才渲染）：「朋友自己建角色」（預設；註「AI 只寫世界與開場，朋友入座時自己建角色」）、「AI 先生好角色給朋友選」。生成中停用；重寫大綱沿用當時的選擇。
- 「朋友自己建角色」：大綱與展開都不含角色——`OutlinePreview` 不畫角色名單、「＋角色」「AI 再加一位」；`canCreate` 照舊只看標題與世界。
- 標題「一句話生成世界開房」、建立鈕「照這份開房」。沿用 `AppDialogs` 那一份常駐 `GenerateTableDialog`（hook 與草稿只留一份）：`open`、`onClose`、`onCreated` 依「是否為開房生成」分流。
- 接線（`useRoomFlows.ts`、`App.tsx`／`AppDialogs.tsx`）：`useRoomFlows` 記住「生成後要開房」與 server；`onCreated(worldId)` 走 existing 分支同一條路（進桌 → pending → `bind(id, server, fresh=false)`），桌名保留生成的標題。對話窗沒建就關（×／Esc）＝取消，回大廳，不建桌、不開房。
- 房間流程其餘不動：公開名冊與歷史在第一次連上時送一次；名單空時朋友入座畫面只剩「建立角色／匯入角色卡」，不加說明文字。

## 2a 提示詞分流（`src-tauri/src/genesis.rs`、`commands/genesis.rs`）

- `generate_table_outline`／`generate_table_expand`（`generate_table_character` 只在生角色時可按）多收一個開房參數（形狀由實作定，例如 `room: Option<{ cast: bool }>`；聊天桌與 VN 傳 null，行為不變）。
- 開房模式共同：把「the player」改成一群玩家——世界段寫「what the players will be doing」，開場旁白對整團說話、停在邀請大家行動的時刻。
- `cast=false`：大綱與展開的輸出結構都拿掉 `## CHARACTER` 段，規則加「Do not write any CHARACTER section; the players bring their own characters」；`parse_outline`／`parse_expand`／`materialize` 接受零角色（確認不會因沒角色而判失敗）。
- `cast=true`：角色規則改成「pre-made player characters a friend can play（一人一位主角，不是配角）；PUBLIC 寫大家看得到的、PRIVATE 寫只有扮演者知道的」。
- 提示詞改動照「可逆小改」：實作後真模型各跑一次（cast 關／開），結果一行記在 §6。

## 3 文案

- `src/i18n/features/room.ts`：新增 `roomFromIdea`「一句話生成世界開房…」；`roomNoCards` 改「不選卡，讓 GM 即興」。
- `src/i18n/<十語>.ts` 一句話開桌那組旁：對話窗標題、建立鈕、二選一兩項與註解（鍵名由實作定）。zh-TW 正典、zh-CN／en 翻譯、其餘七語填英文；`node scripts/check-i18n.mjs` 綠。
- 說明書 `web/src/features/funnel/guide/room.{zh-TW,zh-CN,en}.md` 第 1 節「可以挑幾張角色卡一起開…」那條改成三條路：挑角色卡、用一句話讓 AI 生成世界（預設朋友入座時自己建角色，也可以讓 AI 先生好角色給大家選）、不選卡讓 GM 即興。

## 4 驗證

- `src/features/room/room-flows.test.tsx` 新增：開房窗點「一句話生成世界開房…」→ 一句話對話窗出現、開房窗關掉；沒建就關 → 不建桌不開房（`create_world`、`room_enter` 都沒被呼叫）；`onCreated("w1")` → `room_enter` 用那張桌、`rename_world` 沒被呼叫。既有測試照舊。
- `src/features/lobby/GenerateTableDialog.test.tsx`：開房模式有二選一、預設「朋友自己建角色」且 invoke 帶 `cast: false`、大綱不畫角色區；切到「AI 先生好角色」帶 `cast: true`；聊天桌模式沒有二選一、參數照舊。
- `src-tauri/src/genesis.rs` 測試：開房 `cast=false` 的 outline／expand 提示詞沒有 `## CHARACTER`、有一群玩家的句子；`parse_expand` 零角色可 `materialize`。
- `npm run typecheck && npm test`、`node scripts/check-i18n.mjs`、`node scripts/check-structure.mjs`、`cd src-tauri && cargo test genesis && cargo fmt` 綠；結案前主線 `npm run verify`。

## 5 觀察（範圍外，不立案）

- 多人 GM 提示詞在 `world_md` 空白時沒交代怎麼辦（`src-tauri/src/transport/context.rs`）；本案靠鈕名讓房主知道會即興，不改提示詞。
- 「生成角色但只給 GM 當配角、不可入座」沒做：要動卡旗標（`stage` 語意不同）、名冊與執行端；GM 從世界設定即興配角已夠用。
- 一句話開桌沒有「幾個人玩」欄位；cast=true 時房主要照人數用「AI 再加一位」或手刪。

## 6 施工結果

- 核心約 +250／−15（Rust 提示詞分流與命令 114、前端接線與對話窗 105、CSS 31）、測試 +243、i18n 十語、說明書三語。Grok 施工 3 輪（施工、Sol 修正、樣式）、Sol 驗收 2 輪（第 1 輪必改 2：切模式舊大綱沿用、cast=false 後端沒清模型多寫的角色；第 2 輪 PASS）、Sonnet 測試通道截圖 6 張主線親看。
- 接法：開房生成狀態放 `useRoomFlows.genesisServer`；`AppDialogs` 那份常駐 `GenerateTableDialog` 加 `room` 分流，`room` 變動時清舊大綱（留點子與題材）；invoke 帶 `room: { cast } | null`；後端 `expanded_for_cast` 在 materialize 前把 cast=false 的角色清空。
- 真模型對照（作者正式設定，同一句「一群冒險者在邊境小鎮調查失蹤的商隊」）：cast 關＝世界三段零角色、開場旁白對「你們一行人」說話並停在「你們，要先做什麼？」、桌名用生成標題、入座畫面只剩「建立我的角色卡／匯入角色卡」；cast 開＝四位可扮演的主角（士官、盜賊、法師、牧師）各一句。
- 已知限制：cast 開時模型不知道人數，角色數靠房主增刪（§5）；一句話對話窗在大廳與開房之間共用草稿，切模式會清大綱但點子與題材留著。
