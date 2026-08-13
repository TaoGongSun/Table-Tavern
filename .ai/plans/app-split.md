# App.tsx 拆分計畫

`src/App.tsx` 6890 行，佔前端 9438 行的七成。目的是維修與管理，不追求任何運行時效益；拆分過程不改行為。

目標尺度：一支檔案 200–800 行，`App.tsx` 收到 400–600 行。依據是專案自己的尺度——除 App.tsx 外最大 `refactor-review.ts` 583 行，其餘 400 行以下。

現況：`App()` = 3981–6888（2908 行），內含 59 個 `useState`、13 個 `useRef`、105 個區塊；全檔 145 次 `invoke`、95 個不同指令。每批完成後跑 `node scripts/app-structure.mjs` 重新量測，這是進度的客觀依據。

## 【待拍板】使用者要決定的三件

1. **三批全做，或只做第一批**。第一批風險最低（純搬檔），做完 App.tsx 約 3000 行；三批做完才到 400–600 行。
2. **第二批要不要動**。它是唯一會改結構的一批（state 所有權搬進 controller hook），零元件測試下風險最高。
3. **接受 App.tsx 最終落在 440–570 行**，不是更小。桌次生命週期 9 支 handler（含 `enterTable`）約 166 行必須留在 composition root，這是下限。

## 前置條件（第一批動工前必做）

`scripts/check-i18n.mjs:35` hardcode 只讀 `src/App.tsx`，從中掃 `<button>` 抽 `t("key")`，檢查 10 個語言的按鈕文字寬度是否溢出。目前覆蓋 99 顆按鈕。

元件一搬出 App.tsx，那些按鈕就離開掃描範圍，**腳本會照樣印 OK**——把關失效且無聲。所以：

1. 第 35 行改為掃 `src/` 下所有 `.tsx`。
2. 改完驗證按鈕數仍為 99、全語言全綠（證明覆蓋沒變、只是來源變寬）。
3. 之後每個切片都跑，按鈕數只允許持平或上升，下降即代表漏掃。

## 第一批：機械搬移（App.tsx → 約 3000 行）

每個大元件的 props 已明確（`WorldEditor` 7 個、`CardEditor` 18 個），搬檔不需重新設計介面。

**先做驗證切片**：`CardEditor`＋`CropDialog` 併成單一 `src/views/CardEditor.tsx`（751 行）。它剛好驗證三條規則——私有 helper 跟元件走、跨界 contract 才外抽、微型子元件不另立檔。通過檢查後其餘元件沿同原則完成。

| 產出檔 | 內容 | 行數 |
|---|---|---|
| `views/CardEditor.tsx` | `CardEditor`＋`CropDialog` | ~751 |
| `views/WorldEditor.tsx` | `WorldEditor` | ~1260 |
| `views/SettingsWindow.tsx` | `Settings`＋window wrapper | ~762 |
| `views/UsageTab.tsx` | Usage 工具＋`UsageTab` | ~284 |
| `views/atoms.tsx` | `ErrorNote`、`StoryText`、`EditPane`、`ActReader` | ~130 |

`Onboarding`（63 行）留在 App.tsx；只有第三批做完 App 仍超過 600 行時才搬成 `views/Onboarding.tsx`。

**共用符號歸屬**（按擁有者，不按語法分類；不設 `constants.ts`）：

| 符號 | 落點 |
|---|---|
| `PALETTE`、`Tier`、`CharacterMeta`、`CharacterCard`、`DraftImage` | `src/card-model.ts` |
| `CLI_LABELS`、`tierLabel`、`detectClis`、catalog store、`useModelCatalogs`、prefetch、`CliInfo` | 併入既有 model/CLI 模組 |
| `resolveTheme`、`TEXT_SIZE_PX`、`ThemeId`、主題常數、`KOFI_URL`＋`openSponsorPage()` | `src/appearance.ts` |
| `AppConfig`、`WorldState`、`WorldbookEntry` 等後端資料契約 | `src/` 根層小型 contract 檔 |

元件私有的 props／draft type 跟元件走。不讓 App 反向從 view 檔取 domain type。

## 第二批：controller hook（App.tsx → 約 1500 行）

state 與行為移進 controller custom hook，hook 仍在 `App()` 內按固定順序呼叫，state identity 不變。不用 context、不用 reducer、不引外部 store。跨領域依賴一律用注入的具名 action 表達，controller 之間不互相 import。

| controller | 擁有 | 注入 |
|---|---|---|
| `useTableStateController` | `tableState`、`tableTree`、`tableJumps`、`branchBindings`、`editingStateField`、save／bind／refresh | `worldId`、`onError` |
| `useCharacterController` | `characters`、玩家卡、圖／avatar 快取、GM 圖、載入／增刪／排序／speaker 善後 | `worldId`、`onError` |
| `useImportController` | `importChoice`、`importRoute`、`importReceipts`、`chattedSinceImport`、17 支匯入流程、開場白選擇 | `characters.refresh/hydrate`、`tableState.refresh`、`cardInterface.refresh`、`refreshWorlds`、`enterTable`、`onError` |
| `useChatController` | `events`、`undone`、`generating`、`streamText`、`input`、undo／keepalive ref、對話 handler | `tableState.refresh`、`refreshWorlds`、`imports.noteChatStarted`、`characters.onArrived`、`onError` |
| `useCardInterfaceController` | `cardInterfaces`、`refactorShell`、`cardUiOpen`、殼 memo、message effect | `worldId`、`events`、`tableTree`、`submitText` |

`GenerateTableDialog` 的 10 個 gen* state＋3 支 handler＋137 行 JSX 整組搬成受控元件。

**留在 App**：`worlds`／`table`／`config`／`mainView`、桌次 9 支 handler、`enterTable`、`refreshWorlds`、controller 組裝。

**`generating` 從 chat controller 公開為 `chat.busy`**，桌次操作只讀它。`setWorlds(await list_worlds)` 的重複點統一改呼叫 `refreshWorlds()`——只收斂 action 名稱，不改等待時機、不改後端排序（`src-tauri/src/data.rs:685-719` 依最後活動排序）。

接線順序（避免循環）：root 先有 `worlds`／`table`／`config` 與穩定的 `refreshWorlds` → 注入 chat controller → 拿回 `chat.busy` → 桌次 handler 讀它。

**`enterTable` 的形狀**：保留一次 `read_state` 與既有 transcript／cast 讀取，把同一批 snapshot 分派給各 controller 的 `hydrate`。從 20 個 raw setter 變成約六個語意呼叫，資料欄位對應仍集中可見。

## 第三批：受控 view（App.tsx → 400–600 行）

剩餘 JSX 按畫面抽成受控元件，只吃 controller 的 view model／actions：

- `views/TableSidebar.tsx` — 桌次＋角色側欄（~270 行）
- `views/WorkspaceHeader.tsx` — header＋`StateBar`（~65 行）
- `views/PlayView.tsx` — messages＋composer（~315 行）
- `views/MainView.tsx` — scene／card／world 的 7 值 routing

`mainView` 是 6 個 `kind`＋`null` ＝ 7 種值（`App.tsx:4055-4063`）。新舊角色卡分支的儲存／刪除／離開行為不同（5301-5394、6358-6408），抽介面時不得合併抹掉。

`sidebarWidth`／`tableListOpen`／`stateBarOpen` 三個 localStorage UI state 跟各自的 view 走，不設 layout owner。

## 硬約束

1. **穩定 public API**：controller 對外的 action 用 `useCallback`，公開的 `{view, actions}` 需要時用 `useMemo`，不每次 render 重造整包。注入端的 root action（`refreshWorlds`、`onError`）同樣穩定化；區域 handler 不強制包。
2. **誠實列 reactive dependency**：該隨 `worldId`／資料／action 變化重跑的就放依賴陣列，不得為了安靜藏進 ref。
3. **latest-ref 只用於 event callback 語意**：長駐的 DOM／Tauri listener、timer、channel callback 才用 `App.tsx:4393-4395` 那種模式，並註明為何不重訂閱。
4. **`hydrate` 必須同步**：`hydrate` 只做 state commit，六個之間不得有 await，否則 React batch 被切斷、controller 間出現跨桌混合狀態。`enterTable` 先做完所有 await 再連續同步 hydrate／reset；圖片這類次要資源由該 controller 自己的 effect 延後載入，**且載入前先清掉上一桌的資料**。
5. **不做全域 `api.ts`**：包 95 個指令字串不產生型別安全（型別一樣手寫），只多一層轉接。`invoke` 隨元件／controller 搬走，只有重複或需統一錯誤策略的呼叫才在所屬領域做小 wrapper。

## 目錄結構

```
src/controllers/   第二批的 controller hook
src/views/         第一／三批的 TSX 元件
src/               App.tsx、既有純函式模組、共享 domain 契約
```

兩層到底，不再細分。不為目錄整齊搬動 `model-catalog.ts` 等既有檔（會製造無意義的 import churn）。

## 回歸清單

零元件測試（8 支測試全是純函式，vitest 126 case）是這次拆分的最大風險，這份清單是唯一安全網。

**每個切片**：`npm test`＋`npm run build`＋`npm run check:i18n`，再跑該切片的針對案例，加固定的 A→B→A 換桌檢查。
**每批完成**：跑完整 16 項。第二批全部 controller 完成、第三批完成後各再跑一次；修完失敗項後重跑受影響項與 A→B→A。

| # | 案例 |
|---|---|
| 1 | 換桌 A→B→A：狀態列、角色、訊息、導航都正確，且載入期間不得有舊桌的角色／訊息／狀態樹／圖片閃入 |
| 2 | 匯入角色卡（新桌路徑） |
| 3 | 匯入角色卡（已有匯入紀錄的桌，走路由框） |
| 4 | 匯入世界書 |
| 5 | 開場白選擇＋單條翻譯＋全部翻譯 |
| 6 | 聊天一輪：玩家發言→指定角色回覆 |
| 7 | 換幕＋前幕分岔／退回／重生摘要 |
| 8 | 收回訊息＋復原 |
| 9 | 狀態樹編輯一格＋計數器＋分支綁定 |
| 10 | 卡片介面開關＋殼內按鈕送出 |
| 11 | 設定改主題／字級（含贊助主題鎖） |
| 12 | AI 生成新桌；世界書 AI 重構套用 |
| 13 | 冷啟動回到 `last_world`；無桌時建立範例桌 |
| 14 | 桌次改名／切換／刪除；刪最後一桌補範例桌；AI 回覆後桌次依最後活動重排 |
| 15 | 新建／編輯玩家卡與角色卡、裁圖、封存／刪除、未儲存離開 guard |
| 16 | 復原上次匯入後，角色／訊息／世界設定／卡片介面殼／狀態列同步刷新 |

**切片 → 針對案例**

| 切片 | 案例 |
|---|---|
| `check-i18n` 掃描範圍（前置） | 按鈕數仍 99、全語言全綠 |
| `CardEditor`＋`CropDialog` | 15 |
| `WorldEditor` | 4、12、16 |
| `SettingsWindow`／`UsageTab` | 11 |
| `atoms` | 1、7 |
| `useTableStateController` | 9、16 |
| `useCharacterController` | 15、2、16 |
| `useImportController` | 2、3、4、5、16 |
| `useChatController` | 6、7、8、14 |
| `useCardInterfaceController` | 10、16 |
| `GenerateTableDialog` | 12、13 |
| 第三批各 view | 1、6、7、11、15 |
