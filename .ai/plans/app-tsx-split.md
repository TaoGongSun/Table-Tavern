# App.tsx 拆分立案與計畫

分支：`app-tsx-split`  
立案基準：`main` / `1aaa3fb154ae44e75e6546e10aaa51d4481fccbb`  
原 `src/App.tsx` blob：`a8d65e717bee591a8441caf3a3d95af405d0ac89`  
原檔行數：**1135 行**

## 0. 立案目的

`src/App.tsx` 已經做過第一輪責任下放：聊天、角色、匯入、狀態樹、卡片介面各自有 controller；因此現在剩下的 1135 行，並不是一支單純「什麼都自己做」的舊式元件，而是一個同時承擔 **全域設定、桌次生命週期、跨 controller 協調、主畫面導覽、場景操作與大型 JSX 組裝** 的 composition root。

本案目的不是把 `App.tsx` 變成只剩十幾行，也不是再做一支 800 行的 `useAppController()` 把問題搬家。目標是：

1. 把已經有清楚 owner 的「設定／外觀」、「主欄導覽」、「場景操作」拆成獨立 controller。
2. 把大型 workspace JSX 與全域 dialogs 搬成純 view。
3. **保留真正跨域的協調在 `App.tsx`**：切桌 hydration、匯入復原、AI 重構套用後刷新、sample world 重生等流程，不為了行數硬塞進某一個 feature controller。
4. 拆分後 `src/main.tsx` 仍只 `import App from "./App"`，`App` 無 props、default export 不變。
5. 不改產品行為、不改 Tauri command、不改 CSS class、不改 i18n key、不順手重構現有 controllers。

合理完成線是讓 `App.tsx` 收斂到約 **400–550 行**，其他 implementation 各自約 100–350 行。若 root 最後仍接近 500 行，只要留下的是跨域 coordination，就不再為了數字繼續拆。

## 1. 施工前基準

### 1.1 對外 API

目前 production caller 只有 `src/main.tsx`：

```tsx
import App from "./App";
```

外部契約只有一項：

- `App()` 無 props，default export。

本案保持 `src/App.tsx` 這個入口檔，不搬入口、不改 `main.tsx`。

`App.css` 仍由 `App.tsx` import，避免純拆檔順手改變全域 CSS 載入位置／順序。

### 1.2 目前 1135 行的責任分布

依目前檔案內容，大致可分成下列區塊：

| 區段 | 約略行號 | 責任 |
|---|---:|---|
| imports / types / constants | 1–64 | `UndoReport`、GM target、CLI id、transport helper |
| root state + preferences / DOM effects / bootstrap | 65–240 | config、theme、text size、language、sponsor、首開載入 |
| controller wiring | 241–360 | chat / cardInterface / imports 的依賴接線 |
| `enterTable` + table lifecycle | 361–520 | 切桌 hydrate、建立／刪除／改名／回收桌 |
| scene lifecycle | 521–605 | 換幕、分岔、退幕、重做摘要 |
| export / import undo / post-opening | 606–682 | 跨域匯入復原與逐字稿匯出 |
| card save/removal + leave guard | 683–720 | 卡片善後、未儲存守門 |
| workspace navigation + derived labels + sample regen | 721–859 | 主欄畫面、speaker、場景標籤、設定語言重生 |
| final JSX | 860–1135 | sidebar / header / state bar / MainView / overlays / dialogs |

真正的風險不是 JSX 行數，而是這幾條跨域流程都在同一 closure：

- `enterTable()` 同步 hydrate 5 個 domain state。
- `undoLastImport()` 一次刷新角色、介面殼、GM 圖、聊天、狀態樹、桌名、世界設定、匯入收據。
- `onRefactorApplied` 一次刷新角色、玩家卡、card interface、state、receipts，並重設 speaker。
- `openTableForImport()` 會建立新桌、切桌，再交回匯入 controller。

這些流程本來就屬於 composition root，不能看到它們「很長」就塞進任一單一 feature controller。

### 1.3 現有 controller 依賴順序

目前 wiring 的語意順序是：

```text
App primitive state
  ├─ useTableStateController(table)
  ├─ useCharacterController(table)
  ├─ useChatController(table, scene, characters, tableState)
  ├─ useCardInterfaceController(table, chat, tableState)
  └─ useImportController(table, characters, cardInterface, tableState, App callbacks)
```

其中有一條刻意避免 module cycle 的重要關係：

```text
chat ───────┐
            │ semantic callbacks only
App ────────┼──> imports
            │
cardInterface┘
```

現行註解已明確說明：開場白關閉、聊天開始旗標等留在 `App`，是為了避免 `chat → imports → cardInterface → chat` 繞成環。

拆分時必須維持這個原則：**controller 之間不新增 sibling import；跨 controller 行為由 root 注入 callback。**

### 1.4 現有安全網

固定可跑：

```bash
npm test
npm run build
npm run check:i18n
```

目前沒有完整覆蓋 `App` 所有互動的 component test，因此自動檢查之外必須保留實機回歸清單。

## 2. 拆分原則

1. **root 留 coordination，不留 feature implementation。**
2. **不製造 `useAppController.ts` 巨檔。** 如果一支新 hook 開始同時碰 table/chat/import/character/cardInterface 五域，就表示 owner 劃錯了。
3. **不為了 state 行數建立 state-only wrapper。** `worlds/table/scene/sceneTitles/sceneLabels` 等 primitive session state 可以留在 `App.tsx`；現有 controllers 需要先吃到它們，硬包成另一層只會增加間接性。
4. **view 不持有 domain state。** `AppWorkspace.tsx`、`AppDialogs.tsx` 只接受資料與 action，不自己 invoke、不自己維護桌次／聊天／匯入狀態。
5. **現有 controllers 不在本案二次拆分。** `useChatController`、`useImportController`、`useCharacterController`、`useCardInterfaceController`、`useTableStateController` 視為既有邊界。
6. **Tauri command 字串與執行順序不變。**
7. **不引入 Context、reducer、外部 store、service locator。** 這案只是拆責任，不換狀態架構。
8. **不新增 generic `utils.ts` / `helpers.ts`。** helper 跟真正 owner 走。
9. **不順手改文案、CSS、按鈕順序、modal 疊層、錯誤處理。**

## 3. 定案目錄結構

```text
src/
├── App.tsx                                      # composition root；跨域協調
├── controllers/
│   ├── useAppPreferencesController.ts           # config / language / theme / sponsor / latest config ref
│   ├── useWorkspaceNavigationController.ts      # mainView / speaker / acts / leave guard / 卡片導覽
│   └── useSceneActions.ts                       # 換幕 / 分岔 / 退幕 / 摘要 / 匯出
└── views/
    ├── AppWorkspace.tsx                         # sidebar + header + state bar + MainView + PlayView + card overlay
    └── AppDialogs.tsx                           # GenerateTable / Settings / sample regen / ImportDialogs
```

目標尺度：

| 檔案 | 目標行數 | 責任 |
|---|---:|---|
| `App.tsx` | ~400–550 | session primitive state、controller wiring、切桌 hydration、跨域協調 |
| `useAppPreferencesController.ts` | ~110–160 | config owner 與 DOM preference effects |
| `useWorkspaceNavigationController.ts` | ~150–220 | 主欄導覽、speaker、leave guard |
| `useSceneActions.ts` | ~100–150 | scene lifecycle + transcript export |
| `AppWorkspace.tsx` | ~250–350 | 主工作區 render |
| `AppDialogs.tsx` | ~110–170 | 全域 modal/dialog render |

如果 `AppWorkspace.tsx` 因型別／props 接線自然落在 350 行上下，不再拆一層只包一個元件的薄殼。

## 4. State / responsibility 所有權

### 4.1 `App.tsx` 保留

保留 root/session primitive state：

- `worlds`
- `table`
- `scene`
- `sceneTitles`
- `sceneLabels`
- `editingName`
- `settingsOpen`
- `regenAsk`
- `error`
- `hasStateBar`
- `chattedSinceImport`
- `worldEditorRefreshKey`
- `genTableOpen`

保留現有 domain controller 的 instantiate / wiring：

- `tableState`
- `characters`
- `chat`
- `cardInterface`
- `imports`

保留真正跨域的流程：

- App bootstrap：list worlds + read config + first sample world。
- `adoptImportName()`。
- `openTableForImport()`。
- `resetChatted()`。
- `enterTable()`。
- `switchTable()` / `newTable()` / `deleteTable()` / `renameTable()` / empty-table reclaim。
- `undoLastImport()`。
- `postOpening()` / `postTranslatedOpening()`。
- AI refactor applied 後的跨域 refresh。
- sample world 語言重生最後的「建桌 + enterTable」。

這些功能刻意不再拆成 `useTableLifecycleController`，原因是 `enterTable` 與 `useImportController` 目前有 callback 雙向依賴；硬抽會需要 mutable late-binding ref 或 service locator，得到的間接性比行數收益更差。

### 4.2 `useAppPreferencesController.ts`

搬入：

- `config` / `setConfig`
- `sponsorUnlocked` / `setSponsorUnlocked`
- `language`
- `chatConfigRef`
- `changePreference()`
- `markCliConnectedFromChat()`
- text-size effect
- sponsor status effect
- theme effect
- `document.documentElement.lang` effect
- `transportOf()` 與 CLI id 常數（只在此 domain 使用）

對 root 公開至少：

- `config`
- `setConfig`
- `language`
- `sponsorUnlocked`
- `setSponsorUnlocked`
- `changePreference`
- `markCliConnectedFromChat`
- `transport`
- `currentConfigRef`（若 sample regen 仍需要讀最新值；優先提供語意 getter，避免外部任意改 ref）

**硬約束：**串流期間設定頁可能已經改 config，聊天完成後寫 `cliConnected` 必須讀最新 ref，不能退回 stale closure。

`changeSettingPreference()` 仍留在 App，因為「語言偏好改變 → 是否重生 sample world」已跨到 table lifecycle。

### 4.3 `useWorkspaceNavigationController.ts`

搬入：

- `leaveGuard`
- `canLeaveRef`
- `speaker`
- `mainView`
- `actsOpen`
- `cardView`
- `editingPlayerCard`
- `selectedCard`
- `gmTargeted`
- `canLeaveEditor()`
- `editCard()`
- `openPlayerCard()`
- `selectCard()`
- `selectGm()`
- `openWorldEditor()`
- `openNewCard()`
- `openSceneReader()`
- `deleteCharacter()`
- `deletePlayerCard()`
- card save/removal 後與 `mainView` / `speaker` 有關的善後

它可以依賴：

- `characters` 的語意 actions/data
- `setError`
- `invoke("new_id")`

它**不可以** import chat/import/cardInterface controller，也不處理切桌。

對 root 公開：

- navigation state（`speaker/mainView/actsOpen/...`）
- `setSpeaker` / `setMainView` / `setActsOpen`（root 的跨域 coordination 仍需要）
- `canLeaveEditor`
- `canLeaveRef`
- 上述語意 actions

**leave guard 硬約束：**

1. 角色／玩家／世界編輯畫面離開前仍詢問。
2. 放行後清掉舊 `leaveGuard.current`。
3. `canLeaveRef.current` 每次 render 指向最新 `canLeaveEditor`，讓 `openTableForImport` 的 callback 不吃 stale closure。
4. 切桌後 root 仍負責 `setMainView(null)`、`setActsOpen(false)` 與 card overlay close。

### 4.4 `useSceneActions.ts`

搬入：

- `advanceScene()`
- `forkScene()`
- `revertScene()`
- `regenerateSummary()`
- `exportTranscript()`
- `canUndoScene`
- `sceneDisplayLabel()`

注入語意依賴：

- `worldId`
- `scene`
- `sceneTitles`
- `sceneLabels`
- `tableName`
- `chat`
- `config`
- `canLeaveEditor`
- `enterTable`
- `closeMainView`
- `setError`

不自行持有 scene state，也不直接碰 characters/imports/cardInterface。

**硬約束：**

- `advance_scene` / `regenerate_scene_summary` 前後的 `chat.beginNarration()`、`noteTurnDone()`、`endNarration()` 順序不變。
- fork confirmation 不變。
- `canUndoScene` 仍要求：`scene > 0`、只有一筆 event、not busy、不是 forked scene。
- scene label 的 base/version/title 規則完全不變。
- transcript export 的檔名 timestamp 格式與 `revealItemInDir()` 不變。

### 4.5 `AppWorkspace.tsx`

純 render，搬入目前主 return 中：

- `TableSidebar`
- `WorkspaceHeader`
- `StateBar`
- `MainView`
- `PlayView`
- `ErrorNote`
- `CardInterfaceOverlay`

桌名 rename form 也搬到這支做成受控 UI helper；真正 `renameTable()` command 仍由 App 提供 action。

這支可以接受 grouped controller props，例如 `chat` / `characters` / `tableState` / `cardInterface` / `navigation`，型別用 `ReturnType<typeof useXxxController>` 或專用 view-facing type，不為了 props 數量再把 domain state複製一份。

**禁止：**

- `invoke()`。
- 自己 refresh domain。
- 自己改 localStorage。
- 自己決定切桌／匯入／refactor 善後順序。

### 4.6 `AppDialogs.tsx`

純 render，搬入：

- `GenerateTableDialog`
- `SettingsWindow`
- sample-world regen modal
- `ImportDialogs`

維持現行疊層順序：

1. workspace / card overlay
2. SettingsWindow
3. sample regen modal（在設定視窗之上）
4. ImportDialogs 既有位置與行為

不自行處理 preference、create world、post opening；全部由 action props 注入。

## 5. 最高風險：`enterTable()` hydration 順序

這是本案不可順手「整理漂亮」的區塊。

現行語意：

1. `read_state`
2. `read_transcript`
3. `list_characters`
4. `loadBranchBindings`
5. `scene_appearances`
6. `imports.loadReceipts`
7. `read_world_md` + `read_worldbook`
8. 所有 async 資料讀完後，進入一段**不中途 await 的同步 commit**：
   - `setTable`
   - `setScene`
   - `setSceneTitles`
   - `setSceneLabels`
   - `tableState.hydrate`
   - `chat.hydrate`
   - `characters.hydrate`
   - `imports.hydrate`
   - `setChattedSinceImport`
   - 計算 speaker
   - 清 rename / edit / mainView / acts / card overlay
9. 最後才視需要寫 `last_world` config。

這個設計是為了避免 React batch 被 await 切開，畫面短暫出現「新桌 state tree + 舊桌 transcript」這種跨桌混合。

**拆分時 `enterTable()` 整段留在 App；不拆、不中途插 await、不改 hydrate 相對順序。**

## 6. 其他行為硬約束

### 6.1 speaker / GM

- `GM_TARGET = "__GM__"` 語意不變。
- GM 有世界設定／世界書內容時，切桌預設指 GM。
- GM 全空時，預設第一張本幕可見角色；沒有角色仍回 GM。
- `undoLastImport` 刪掉目前 speaker 時切回 GM。
- refactor applied 後一律切回 GM。
- 聊天畫面點同一張卡是取消發言對象；編輯畫面點卡是導覽。

### 6.2 undo import

`undoLastImport()` 保留 root，刷新順序與條件不變：

- undo command
- characters refresh
- speaker fallback
- card interfaces
- card shell
- GM image
- removed opening 時 chat + tableState
- worlds
- world editor remount key
- receipts
- 完成訊息

`chattedSinceImport` 的 localStorage key 與「一發出 AI 對話就不再允許復原」規則不變。

### 6.3 table lifecycle

- 首開沒有桌 → 依語系建立 sample world。
- 仍保證 App 不進入永久零桌狀態。
- `last_world` 恢復規則不變。
- 空白且仍是自動命名的桌，離開時可回收。
- 使用者改過名字的空桌不回收。
- 「開新桌並匯入」先問 leave guard，再建桌，再切桌；取消不能留下半張新桌。

### 6.4 settings / language

- 語言改變立即寫 config。
- sample regen 一生只問一次的 `sample_regen_asked` 規則不變。
- cancel 會把語言退回原值且不算問過。
- keep 只記 asked。
- regen 會先過 leave guard，再建立新 sample world 並進桌。

### 6.5 render / overlay

- card interface overlay 只在 `mainView === null && uiOpen && shellReady` 出現。
- 編輯畫面不另外手動清 `uiOpen`，靠 render gate 消失。
- StateBar 仍只在 mainView null 且 `hasStateBar || tree non-empty` 時 render。
- ErrorNote transport 判斷不變。

## 7. 施工順序（約 20 分鐘自然工作段）

每段都應可以獨立停下、commit、回報；不要一次拆完 1135 行才驗證。

### 工作段 A：baseline + preferences controller

- 再確認 `main` / branch HEAD 與 App blob。
- 建 `useAppPreferencesController.ts`。
- 搬 config/sponsor/language/theme/text-size/latest-ref 邏輯。
- App bootstrap 改接 controller 暴露的 `setConfig`。
- 跑 automated checks。

風險低，先建立拆 hook 的型別與 import 模式。

### 工作段 B：workspace navigation controller

- 建 `useWorkspaceNavigationController.ts`。
- 搬 mainView/speaker/acts/leave guard 與卡片導覽 actions。
- App 的 chat/import/table lifecycle 改接 navigation 公開的 state/action。
- 特別驗證 `canLeaveRef.current` 與 new-card `new_id`。
- 跑 automated checks + 卡片 leave guard 實測。

### 工作段 C：scene actions

- 建 `useSceneActions.ts`。
- 搬 advance/fork/revert/regenerate/export/labels。
- 不動 `enterTable()` 本體。
- 跑 automated checks + scene 實測。

### 工作段 D：workspace view

- 建 `AppWorkspace.tsx`。
- 搬 sidebar/main/header/state/MainView/PlayView/card overlay JSX。
- 搬受控 rename form markup。
- 不移任何 invoke/domain refresh。
- 跑 automated checks + 主要畫面導覽實測。

### 工作段 E：dialogs view + root 收斂

- 建 `AppDialogs.tsx`。
- 搬 generate/settings/regen/import dialogs。
- App 只留下狀態與 actions。
- 核對 modal 疊層與 backdrop 行為。
- 跑 automated checks。

### 工作段 F：完整回歸與結構複核

- 全部 automated checks。
- 實機回歸第 9 節。
- 檢查 Tauri command 字串集合前後無增減。
- 檢查 `enterTable` hydration order 無變。
- 檢查 controller sibling imports 無新增 cycle。
- 統計最終各檔行數。
- 把實作結果補回本文件。

如果某一段實際很快完成，可以把下一個低風險相鄰段落一起做；但 `enterTable`、undo import、leave guard、sample regen 這幾個高風險邊界不要跨段一路改到底才驗證。

## 8. 每個工作段的自動驗收

固定跑：

```bash
npm test
npm run build
npm run check:i18n
```

結構核對：

1. `src/main.tsx` 不需要修改。
2. `App` default export / 無 props 不變。
3. `App.css` import 仍存在且載入順序不變。
4. Tauri command 名稱集合不增不減。
5. localStorage key `chatted_since_import:${worldId}` 不變。
6. 不新增 React Context / reducer / external store。
7. 現有五支 controller 不因本案混入新的跨域責任。
8. view 檔不得出現 `invoke(`。

## 9. 實機回歸清單

### 9.1 啟動 / 桌次

- 有既有桌時回到 `last_world`。
- 建新桌、切桌、刪桌、改名。
- 自動名空桌離開後可回收。
- 手動改過名字的空桌不被回收。
- 刪掉最後一桌後自動補 sample world。

### 9.2 導覽 / leave guard

- 聊天畫面點角色：選／取消 speaker。
- 編輯畫面點角色：改成導覽另一張卡，不改 speaker。
- GM 卡聊天／編輯兩種語意。
- 新角色卡／新玩家卡取得 id 並可存檔。
- 角色卡、玩家卡、世界設定有未儲存內容時，切畫面／切桌會正確詢問。

### 9.3 scene

- advance scene。
- 剛換幕時 revert scene。
- regenerate summary。
- fork historical scene。
- forked scene 不誤開 undo-scene 補救。
- scene title/version label 顯示不變。
- transcript export 可另存並 reveal。

### 9.4 import / undo

- 第一張卡匯入後自動命名桌的 adopt-name 行為。
- 自訂桌名不被 import 改掉。
- 第二張卡路由。
- 「開新桌並匯入」取消 leave guard 時不建立桌。
- 匯入後未聊天可 undo。
- 一發出 AI request 後 undo import 入口收起。
- undo 後角色、speaker、GM image、card interface、state、world editor、receipts 同步刷新。

### 9.5 chat / card interface / refactor refresh

- GM target narration 與角色 target reply 正常。
- card interface overlay 開關正常，切編輯畫面時不顯示。
- refactor apply 後角色名單、玩家卡、card shell、state、receipts 重讀，speaker 回 GM。

### 9.6 settings / language

- 文字大小即時生效。
- theme / sponsor 狀態正常。
- 設定頁改 AI config 後聊天收尾不會用 stale config 覆蓋。
- 第一次改語言 sample regen：cancel / keep / regen 三條都正確。
- `sample_regen_asked` 後續不再重問。

## 10. 明確不做

本案不包含：

- 拆 `useChatController.ts`。
- 拆 `useImportController.ts`。
- 拆 `useCharacterController.ts`。
- 拆 `useCardInterfaceController.ts`。
- 拆 `useTableStateController.ts`。
- 改 App 的狀態架構成 Context / reducer / Zustand 等。
- 改任何 Tauri command contract。
- 改 CSS / UI 設計。
- 改 i18n 文案。
- 改 sample world、scene、import、refactor 的產品規則。
- 為了讓 `App.tsx` 低於某個漂亮行數，把跨域 coordination 再硬搬一次。

## 11. 完工定義

本案完成需同時滿足：

1. `App.tsx` 明顯收斂，留下的主要內容可被描述為「session composition + cross-domain orchestration」。
2. 三個新 controller 各自只有單一清楚 owner，不互相 sibling-import。
3. 兩個新 view 無 domain side effect。
4. `enterTable()` 的 async load → synchronous hydrate 邊界完全保留。
5. undo import / refactor applied / leave guard / sample regen 的跨域語意不變。
6. `npm test`、`npm run build`、`npm run check:i18n` 全綠。
7. 第 9 節實機回歸完成，沒有因拆檔造成行為回歸。
8. 最終檔案行數與任何與本計畫不同的實際切法，都補記到本文件「施工結果」。

## 12. 施工結果

### 12.1 最終行數

| 檔案 | 行數 |
|---|---:|
| `src/App.tsx` | 612（原 1135） |
| `src/views/AppWorkspace.tsx` | 345 |
| `src/controllers/useWorkspaceNavigationController.ts` | 213 |
| `src/controllers/useSceneActions.ts` | 160 |
| `src/views/AppDialogs.tsx` | 119 |
| `src/controllers/useAppPreferencesController.ts` | 99 |

`App.tsx` 612 行高於第 0 節寫的 400–550〔作者裁決 2026-09-06：接受，不再拆〕。留在 root 的是切桌 hydration、桌次生命週期、匯入復原、重構後刷新、語言重生，都是第 4.1 節指定要留下的跨域協調。

### 12.2 與計畫不同的實際切法

- `chatConfigRef` 改名 `currentConfigRef`，搬進 preferences controller 並對外暴露，供 root 的 `changeSettingPreference` 與 `answerRegen` 讀取。
- `useSceneActions` 是純函式而非 hook（內部不用任何 React hook），`setMainView(null)` 改由 root 注入 `closeMainView` 回呼。
- 生桌對話框的 DOM 位置從版面最前移到卡片介面浮層之後。`.modal-overlay`（z-index 10）本來就高於 `.card-interface-overlay`（9），疊層順序不受影響。

### 12.3 靜態驗收

`npm test` 157 passed、`npm run build`、`npm run check:i18n` 九語全綠。

逐字比對 main 與本分支：`enterTable` 54 行、`undoLastImport` 42 行完全相同；9 個子元件共 188 個 prop 名單一致，差異只有轉手改名；全 `src` 36 個 Tauri command 不增不減；`main.tsx` 未改動；`App.css` 仍是最後一個 import；三支新 controller 彼此零 import；兩支新 view 零 `invoke(`。

### 12.4 實機回歸（2026-09-06，release 包）

第 9 節清單全部通過，畫面與磁碟一致，未發現任何因拆檔造成的行為改變。含守門三入口（點卡／切桌／換幕）與取消、確定兩條分支；換語言取消／保留／重生三條路；換幕、退幕、重寫摘要、分岔與分岔幕不誤開 undo 補救；匯入身分框、第二張卡路由、開新桌並匯入取消時不留半張桌、復原後六個域同步退回（含開場白從逐字稿收回）；桌次建立、切換、改名（標題列與側欄兩個入口）、刪除、自動名空桌回收與手動改名空桌不回收；串流期間改 AI 設定不被收尾覆蓋；逐字稿匯出。

兩項未驗：

- AI 重構套用後的刷新〔作者裁決 2026-09-06：本案跳過〕。
- 「浮層在 `mainView` 非 null 時不顯示」無法在真實操作下構造（浮層整面覆蓋，沒有切往編輯畫面的入口）；已驗等價路徑：進編輯畫面後標題列的卡片介面入口消失，浮層開不起來〔作者裁決 2026-09-06：標為已驗〕。
