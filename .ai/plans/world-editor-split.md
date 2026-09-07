# WorldEditor.tsx 拆分立案與計畫

分支：`world-editor-split`  
立案基準：`main` / `010a923adde7750fb8a48e783f15509552a51300`  
原 `src/views/WorldEditor.tsx` blob：`2c57514af333499bda1a6cd59384e977a7ccc44f`

## 0. 立案目的

`src/views/WorldEditor.tsx` 目前約 **1519 行**。它原本是 `App.tsx` 第一輪拆分時搬出的單一大型 view，但後續世界書、機制帳本與 AI 卡重構功能都持續長在同一支元件內，現在已再次成為前端的大型高耦合檔。

本案目的只做 **責任拆分與可維護性改善**，不改產品行為、不改後端契約、不順手重寫 AI 重構流程。拆完後 `MainView.tsx` 仍只 import `./WorldEditor`，`WorldEditor` 對外 props 與既有 remount／leave guard 語意全部保留。

本案不是追求「檔案越多越好」，而是把目前已經自然形成的責任邊界拆開，讓每支 implementation 大致落在 100–500 行、單一 state owner 清楚、AI 流程與世界書 CRUD 不再塞在同一個 render body。

## 1. 施工前基準

### 1.1 對外 API

目前 production caller 只有 `src/views/MainView.tsx`，`WorldEditor` 對外維持以下 7 個 props：

1. `world`
2. `worldName`
3. `onBack`
4. `leaveGuard`
5. `convertColor`
6. `onEntryConverted`
7. `onRefactorApplied`

本案不改這 7 個 props，不要求 `MainView.tsx` 跟著拆分，也不把 WorldEditor 的內部 state 提升到 App/MainView。

`MainView` 目前用 `key={worldEditorRefreshKey}` 強制整支 WorldEditor remount，以處理復原匯入後的世界書重載；拆後這把 key 仍掛在最外層 `WorldEditor`，因此所有子 hook／子元件都必須一起重建，不能把任何 feature state 搬到模組級 singleton 或外部 store。

### 1.2 目前單檔的主要責任

目前內容可分成五群：

1. **世界設定 `world.md`**
   - `read_world_md` / `write_world_md`
   - `text` / `savedText` / `message`
   - 返回與未儲存提示

2. **世界書 CRUD + 可見性 + 排序**
   - `read_worldbook`
   - 新增／編輯／刪除／排序／去重／匯出
   - 條目 draft、characters visibility、角色清單刷新
   - `worldbook_entry_to_character`

3. **機制帳本**
   - `mechanism_ledger`
   - accepted/skipped 等記錄顯示
   - 「照原文送模型」切換，實際重用 `upsert_worldbook_entry`

4. **AI 卡重構 workflow**
   - 三態判定與 recommend
   - survey → local assembly → bounded parallel expand
   - rate-limit retry、取消、abort sentinel、partial failure
   - 匯入／匯出重構產物、apply、apply 後刷新

5. **大型 JSX**
   - 世界書條目 inline form
   - 世界書列表與 ledger
   - AI progress modal
   - 模式二選一 modal
   - 結果摘要／詳細選擇 modal

目前一支 `WorldEditor()` 同時持有約 18 個 `useState`、多個 ref／effect，並直接橫跨約 25 個 Tauri command 名稱。最大風險不是單純行數，而是「世界書 dirty/leave 語意」與「AI 重構長 async workflow」共用同一個 closure 空間，修改任一區都容易碰到另一區。

### 1.3 現有安全網

- `npm run build`：TypeScript + Vite build
- `npm test`：Vitest；AI 重構的純資料操作已有 `src/features/refactor/refactor-review.test.ts` 等測試
- `npm run check:i18n`

目前沒有針對 `WorldEditor` 的完整 component interaction test，因此本案不能只靠編譯綠燈；需要固定實機回歸清單。

## 2. 拆分原則

1. **維持外部介面**：`src/views/WorldEditor.tsx` 保留，仍 export `WorldEditor`；`MainView.tsx` 不因本案改接線。
2. **state 跟責任 owner 走**：不是把 JSX 搬檔後留下 40 個 props，而是將「世界書」與「AI 重構」各自的 state/action 搬進 feature hook。
3. **root 只做跨責任協調**：`WorldEditor.tsx` 保留 world.md、總 leave guard、兩個 feature 的組裝；不重新長成另一個 controller 巨檔。
4. **不引入 Context / reducer / 外部 store**：本案規模不需要新增全域狀態層。
5. **不新增 generic `utils.ts` / `constants.ts`**：helper、map、type 留在真正 owner；只有確實被 worldbook hook 與其 UI 同時使用的 model 才放共用檔。
6. **不搬既有純 refactor domain helper**：`refactor-review.ts`、`refactor-run.ts`、`refactor-mode.ts` 已經是合理邊界，本案只移動它們在 UI workflow 中的 orchestration。
7. **不順手改 UI 文案／CSS class／後端命令／錯誤字串**。

## 3. 定案目錄結構

```text
src/views/
├── WorldEditor.tsx                         # composition root，對外 API 不變
└── world-editor/
    ├── worldbook-model.ts                  # WorldbookDraft / Ledger 等 feature contract
    ├── useWorldbookEditor.ts               # 世界書 state + CRUD + ledger + cast
    ├── WorldbookSection.tsx                # 世界書 actions / ledger / list
    ├── WorldbookEntryForm.tsx              # inline 新增／編輯表單
    ├── useRefactorWorkflow.ts              # AI 重構 state + async orchestration
    ├── RefactorRunDialogs.tsx              # progress + 模式二選一
    └── RefactorResultDialog.tsx             # 結果摘要／詳細選擇／套用 UI
```

目標尺度不是硬限制，但施工完成後希望：

| 檔案 | 目標行數 | 責任 |
|---|---:|---|
| `WorldEditor.tsx` | ~180–260 | world.md、leave guard、feature 組裝 |
| `worldbook-model.ts` | ~50–100 | 世界書 feature 共用型別／空 ledger |
| `useWorldbookEditor.ts` | ~260–340 | 世界書資料與行為 owner |
| `WorldbookSection.tsx` | ~250–330 | 世界書／ledger 清單 UI |
| `WorldbookEntryForm.tsx` | ~130–180 | draft 表單 UI |
| `useRefactorWorkflow.ts` | ~400–500 | AI workflow owner |
| `RefactorRunDialogs.tsx` | ~100–180 | 跑動中／選模式 dialog |
| `RefactorResultDialog.tsx` | ~300–380 | 結果審核 dialog |

如果實際搬移後某個自然責任落在 500 行上下，不為了湊數再切微型檔；反過來若 `useRefactorWorkflow.ts` 明顯超過 550–600 行，才考慮把「檔案 import/export/apply」與「AI run orchestration」再拆成兩個語意明確的 hook/module。

## 4. State 所有權

### 4.1 `WorldEditor.tsx`

只保留：

- `text`
- `savedText`
- `message`
- `read_world_md` / `write_world_md`
- 最外層 loading gate
- `confirmLeave()` / `leaveGuard.current`
- `onBack`
- worldbook/refactor feature 組裝

`unsavedCount` 仍由 root 計算，因為它同時包含：

- world.md 未儲存：`text !== savedText`
- 世界書「尚未存過的新 draft」：`worldbook.newEntryDirty`

這個跨責任計數就是 root 應該保留的協調邏輯。

### 4.2 `useWorldbookEditor.ts`

搬入：

- `entries`
- `ledger`
- `characters`
- `worldbookMessage`
- `draft`
- `draftOrigin`
- refresh cast/worldbook/ledger
- ledger toggle
- draft open/close/add/edit/persist/save/delete
- reorder/dedupe/export
- entry → character conversion

對 root 至少公開：

- `entries`
- `ledger`
- `characters`
- `draft`
- `draftOrigin`
- `draftDirty`
- `newEntryDirty`
- `message`
- CRUD/actions
- `refreshWorldbook()` / `refreshLedger()` / `refreshCast()`
- `flushExistingDraftForLeave()`

`flushExistingDraftForLeave()` **只處理目前已存在（uid != null）的 dirty draft：先自動存，成功後關閉；新 draft 不在這裡自己跳第二個 confirm**。這是為了保留現行 leave 行為：world.md 與新 draft 最後一起由 root 的 `unsavedLeaveConfirm(n)` 問一次。

### 4.3 `useRefactorWorkflow.ts`

搬入目前所有 `refactor*` state/ref 與 handler：

- outcome / selection / origin / detail / cancelled / mode ask
- failures / busy / progress / cancel ref
- `runAiRefactor()`
- `startRefactorRun()`
- `cancelAiRefactor()`
- import outcome
- close / restore dropped
- apply
- export current outcome / export saved outcome
- summary message helper

它從 root 注入的依賴只保留語意 action，不直接碰 WorldEditor 其他 state：

- `world`
- `worldName`
- 最新 `entries`
- `setStatusMessage(message)`
- `refreshAfterApply()`

其中 `refreshAfterApply()` 由 root 組合，並維持現行順序：

1. refresh worldbook
2. refresh ledger
3. refresh cast
4. `onRefactorApplied()`
5. 顯示 apply summary message

不要讓 refactor hook import worldbook hook，也不要讓 worldbook hook import refactor hook；兩者都只由 root 組裝，避免 sibling cycle。

## 5. UI 拆法

### 5.1 `WorldbookEntryForm.tsx`

純受控表單，接受 draft、characters 與 action props。保留：

- 原按鈕順序
- visibility 三種 radio
- 點「指定角色」時重新抓角色的既有語意（由 action 提供）
- 新增表單排列表底部、編輯表單原地取代該 row
- `scrollIntoView({ block: "nearest" })` 行為

表單不自己持有 domain state，避免同一份 draft 在 parent/hook 與 form 各有一份。

### 5.2 `WorldbookSection.tsx`

負責：

- action row
- status message
- mechanism ledger
- `useDragReorder`
- worldbook list
- inline `WorldbookEntryForm`

AI 重構按鈕仍留在同一 action row，但只吃 refactor workflow 暴露的 actions/status，不把 AI orchestration 寫回這個 component。

### 5.3 `RefactorRunDialogs.tsx`

只放：

- progress modal
- stream tail
- cancel button
- recommend／自己選 dialog

保持目前「recommend 失敗時不偽造 evidence、直接展開兩選項且介面優先」的 UI 行為。

### 5.4 `RefactorResultDialog.tsx`

完整搬出結果摘要與詳細審核 UI：

- cancelled / partial failure 標題與紅字
- summary counts
- character / entries / interface / mechanisms selection
- suspected player toggle
- dropped restore
- unabsorbed / audit 顯示
- export / dismiss / apply all / detailed apply

`REFACTOR_DROPPED_RULE_KEYS` 與 `REFACTOR_AUDIT_KIND_KEYS` 只在這支檔案使用，就留在這支檔，不另造 constants 檔。

## 6. AI 重構流程硬約束

這區是本案最高風險，拆檔只允許搬 owner／接線，不改演算法。

以下全部 byte-level 語意保留：

1. `refactor_outcome_exists` rerun warning。
2. `card_interfaces` → `detectRefactorTristate()` 三態分流。
3. `none` 直接走 characters；`unsupported` 擋下；`supported` 先 recommend。
4. recommend 成功才保留 resume ticket；失敗時 ticket = null。
5. survey → `refactor_assemble_local` → `buildRefactorPersonPlan` 順序不改。
6. `knownFields = survey.fields` 固定共用，不沿呼叫鏈累積。
7. pool 內容與 `REFACTOR_PARALLEL_LIMIT` 不改。
8. `runRefactorCalls({ chain: [], warmed: true })` 不改。
9. `withRateLimitRetry` 的取消 predicate 不改。
10. abort sentinel `refactor-aborted` 仍靜默略過，不列 failure。
11. mode mismatch 仍轉成 `refactorModeMismatch`。
12. partial failure 仍收集 name + reason，且結果 modal 仍可開。
13. cancelled 半成品仍可顯示，但主按鈕語意不改。
14. `recordReceipt: refactorOrigin !== "ai"` 不改。
15. 結果 modal 點 backdrop 不關閉。

拆分 PR/diff 若出現上述邏輯的「順便簡化」，視為超出本案，應退回原寫法。

## 7. Leave / dirty 行為硬約束

這區是第二高風險。

現行規則必須逐條保留：

1. world.md dirty → 離開時詢問。
2. 既有 worldbook entry dirty → 切換編輯對象或離開時自動存；存失敗就不離開。
3. 新 worldbook entry dirty → 不自動留下半成品，離開時與 world.md dirty 一起算進單一 `unsavedCount`。
4. 表單按取消 → dirty 時單獨詢問。
5. `leaveGuard.current` 每次 render 都必須指向能看到**最新** dirty state 的函式；不要為了「穩定 reference」把舊值藏進 stale callback。
6. `WorldEditor` remount 時所有 dirty/refactor/draft state 一次重設。

## 8. 施工順序（約 20 分鐘自然工作段）

每段都要能停下並回報，不一次把 1519 行全吞完。

### 工作段 A：baseline + model + 表單 view

- 再量一次 `WorldEditor.tsx` 行數與主要 handler 邊界。
- 建 `world-editor/worldbook-model.ts`。
- 搬 `WorldbookEntryForm.tsx`。
- 不移 state owner。
- 跑 build/test/check:i18n。

這段的目的先驗證「JSX 搬檔＋type plumbing」沒有破壞 i18n / TSX。

### 工作段 B：世界書 hook

- 建 `useWorldbookEditor.ts`。
- 搬 entries/ledger/cast/draft 與 CRUD actions。
- 實作 `flushExistingDraftForLeave()`。
- root 接回 `newEntryDirty` 與 leave guard。
- 跑 automated checks + 世界書 dirty/CRUD 實測。

### 工作段 C：世界書 section

- 建 `WorldbookSection.tsx`。
- 搬 ledger/list/action row/drag reorder。
- 確認 inline form 位置與拖曳 row 行為不變。
- 跑 automated checks + reorder/ledger/convert 實測。

### 工作段 D：refactor workflow owner

- 建 `useRefactorWorkflow.ts`。
- 先只搬 state + async handlers，結果 modal JSX 暫時仍可留 root。
- 逐條核對第 6 節 15 項硬約束。
- 跑 automated checks。

### 工作段 E：refactor dialogs

- 搬 `RefactorRunDialogs.tsx`。
- 搬 `RefactorResultDialog.tsx`。
- root 收斂成 composition。
- 跑 automated checks + 零 AI 額度的 import outcome 實測。

### 工作段 F：完整回歸與收尾

- 完整 automated checks。
- 實機回歸清單全跑。
- 至少一次 live AI 重構流程＋一次取消流程。
- 檢查 `MainView.tsx` 無需改動。
- 檢查沒有多出的 generic helper / context / store。
- 統計最終各檔行數並補回本文件「施工結果」。

如果某工作段實際很快完成，可把下一個相鄰切片併入同一自然工作段，但不要跨越未驗證的高風險邊界直接一路拆到底。

## 9. 每個切片的自動驗收

固定跑：

```bash
npm test
npm run build
npm run check:i18n
```

另外做結構核對：

1. `MainView.tsx` 對 `WorldEditor` 的 7-prop 接線不變。
2. 拆前後 Tauri command 名稱集合不應增減；本案不得改 command 字串。
3. CSS class 名稱不因拆檔改名。
4. i18n key 不因拆檔改名／新增替代文案。
5. `refactor-review.ts` / `refactor-run.ts` / `refactor-mode.ts` 不因本案被重寫。

## 10. 實機回歸清單

### 不需 AI 額度

1. 進入世界設定、無修改直接返回。
2. 改 world.md → 返回 → 取消／確認兩條路都正確。
3. 編輯既有世界書條目後切到另一條：自動存成功；存失敗時不可切走。
4. 新增世界書條目但未儲存 → 返回：只出既有單一未儲存確認；取消後 draft 仍在。
5. 新增／編輯／刪除世界書條目。
6. 拖曳排序後重開世界設定，順序有落地。
7. 去重、匯出世界書。
8. visibility 切到「指定角色」，角色名單可刷新、勾選可存回。
9. ledger 顯示與「照原文送模型」切換。
10. 世界書條目轉角色卡，轉完清單刷新且 `onEntryConverted()` 善後正常。
11. 匯入一份既有 refactor outcome JSON：摘要、展開、勾選、玩家指定、dropped restore、audit/unabsorbed 都可看。
12. 匯出目前 refactor outcome、關閉結果、不套用。
13. 套用匯入的 refactor outcome：worldbook/ledger/cast/App 層資料全部刷新，summary message 正常。
14. 復原匯入造成 `worldEditorRefreshKey` 變化時，整支編輯器 remount，不殘留舊 draft／modal／progress。

### 需要 live AI

15. supported card：recommend → 直接照建議跑完，結果可開。
16. supported card：展開「自己選」後改另一模式，ticket／mode 行為正常。
17. 跑動中取消：取消鈕 disabled 狀態、abort、partial/cancelled 結果呈現正常，沒有把 `refactor-aborted` 當紅字 failure。

## 11. 明確不做

本案不做：

- 不重寫 AI prompt／refactor algorithm。
- 不調整 parallel limit 或 retry policy。
- 不改後端 Rust。
- 不改 worldbook schema / visibility schema。
- 不把 invoke 包成全域 API layer。
- 不新增 Context、Redux/Zustand 類 store、reducer。
- 不改 CSS 視覺設計。
- 不改 i18n 文案。
- 不順手處理 WorldEditor 之外的大檔。
- 不以「順便抽共用」為理由碰 CardEditor / MainView。

## 12. 完成定義

本案完成時應同時滿足：

1. `WorldEditor.tsx` 不再是 >1000 行巨檔，且 root 只負責 world.md + 跨 feature 協調。
2. 世界書與 AI refactor 各有單一清楚 state owner。
3. 沒有一串 30–40 props 的假拆分。
4. `MainView` caller 與 7 個外部 props 不變。
5. 第 6、7 節硬約束全部核對通過。
6. `npm test`、`npm run build`、`npm run check:i18n` 全綠。
7. 第 10 節實機回歸完成。
8. 最終 diff 不含本案以外的功能／命名／樣式重構。

## 13. 施工結果

commit：`30f1437`（立案）→ `90523d9`（composition root 收尾），分支 `world-editor-split` 共 8 筆，工作段 A–F 各自成 commit。

### 13.1 最終行數

| 檔案 | 計畫目標 | 實際 |
|---|---:|---:|
| `WorldEditor.tsx` | 180–260 | 132 |
| `world-editor/worldbook-model.ts` | 50–100 | 40 |
| `world-editor/useWorldbookEditor.ts` | 260–340 | 316 |
| `world-editor/WorldbookSection.tsx` | 250–330 | 238 |
| `world-editor/WorldbookEntryForm.tsx` | 130–180 | 132 |
| `world-editor/useRefactorWorkflow.ts` | 400–500 | 528 |
| `world-editor/RefactorRunDialogs.tsx` | 100–180 | 98 |
| `world-editor/RefactorResultDialog.tsx` | 300–380 | 398 |

拆前 1519 行；拆後 root 132 行，八檔合計 1882 行（多出來的是 props 介面與 import）。

### 13.2 偏離計畫之處

- root 132 行低於目標下限：world.md 那段本來就只有一個 textarea 與存檔，計畫高估。
- `useRefactorWorkflow.ts` 528 行、`RefactorResultDialog.tsx` 398 行都略高於目標，仍在第 3 節「落在 500 上下不再切微型檔」的容許範圍，未觸發 550–600 的再拆條件。
- 兩處無條件搬移的順手處理：EntryForm 的「指定角色」重抓角色改叫 hook 的 `refreshCharactersForVisibility()`（原為 inline invoke，語意與無錯誤處理照舊）；`confirmLeave` 裡的既有條目自動存改叫 `flushExistingDraftForLeave()`。

### 13.3 驗收結果

機械關卡（2026-09-06，macOS）：`npm test` 157 passed／11 files、`npm run build`、`npm run check:i18n` 全綠。

結構核對：

- 25 個 Tauri command 名稱集合拆前拆後相同。
- 116 個 i18n key 相同，連各 key 出現次數都一致。
- 所有 `className` 字串與出現次數一致。
- `MainView.tsx` 零改動，7 個 props 不變。
- `refactor-review.ts` / `refactor-run.ts` / `refactor-mode.ts` 未被本案碰到。
- 沒有新增 Context／store／generic helper 檔。

第 6 節 15 條 AI 硬約束、第 7 節 6 條 leave/dirty 硬約束逐條核對通過；差異只有變數改名（`refactorProgress`→`progress`、`setWorldbookMessage`→`setStatusMessage`、setState updater 參數 `selection`→`current`）與換行排版，無邏輯改寫。

`useDragReorder` 由 root 搬進 `WorldbookSection`：該 hook 只有 state 與 pointer handler、無 effect 與訂閱，且 preview 在 pointerup 即清空，搬層不改行為。

### 13.4 實機回歸結果（2026-09-06，release 包，測試桌「新的一桌 6」）

第 10 節免 AI 額度那批，過的項目：

1. 進入世界設定、無修改直接返回：不彈確認，直接回聊天畫面。
2. 改 world.md → 返回：確認框出「有 1 項修改未儲存」；取消留在原地且文字還在，確認離開後修改沒落地。
3. 編輯既有條目改標題後直接點另一條的編輯：不問、自動存，清單即時顯示新標題。（「存失敗就不能切走」造不出寫入失敗，未驗）
4. 新增條目未存 → 返回：只出單一「1 項未儲存」確認；取消後 draft 與內文都還在。表單按取消同樣單獨詢問。
5. 新增／編輯／刪除條目都正常，刪除有二次確認。
6. 拖曳把第二條拉到第一條之上，返回再進來順序仍在。
7. 清理重複：無重複時回「沒有重複條目」；匯出世界書落檔 `TestCards/worldbook-split-test.json`，內容為 ST 格式且條目正確。
8. 可見範圍切「指定角色」當下重抓角色清單（抓到剛轉出的角色卡），勾選後儲存，清單 badge 顯示「1 位角色」。
10. 條目轉角色卡：出「已轉成角色卡「條目B」，原條目已刪除」，清單與側欄都刷新。

另驗：「匯出重構卡」在沒有已套用產物時回「這桌還沒有重構卡」，存檔預設檔名帶桌名前綴。

第 11–13 項（匯入既有產物 → 人審 → 套用，用 `TestCards/新的一桌 7-重構卡.json`，18 角色／34 條目／有介面）：

- 結果視窗摘要行「拆出 18 個角色・遊戲介面搬進 app・重建 34 條世界書條目」，機制 0 筆不列。
- 點視窗外不關閉（第 6 節硬約束 15）。
- 展開細看：角色列表 18 筆、展開單一角色看得到「出處」與公開／私有全文；世界書條目 34 筆帶 kind badge；介面區展開顯示出處與五個欄位名。
- 勾選：取消勾一個角色與一條條目後套用，落地 17 張角色卡、33 條條目，被取消的兩筆都不在磁碟上——selection 確實傳到後端。
- 匯出目前產物：落檔 `TestCards/split-test-outcome.json`，內容與來源卡一致（匯出整份產物，不受勾選影響），存完自動在 Finder 顯示。
- 套用後 worldbook 清單、角色清單、App 層都刷新，摘要對話框正常。

背景控制（app_* 工具）點不到這個 HTML modal 的按鈕，改用全螢幕控制才跑得動；原生存檔／確認對話框則兩種模式都可點。下一輪要自動化這段時直接走全螢幕。

第 14 項（復原匯入 → `worldEditorRefreshKey` 變化 → 整支 remount）：先開著一條既有條目的編輯表單，再按側欄「復原上次匯入」。確認框正確認出這筆是「AI 卡重構」的匯入；復原完成後編輯表單消失、世界書清單重載成空、側欄角色卡收回，沒有殘留 draft／modal／progress。磁碟上角色卡 0 張、世界書 0 條。

未驗：第 9 項機制帳本（測試桌沒有機制條目）、玩家卡指定（這張卡沒有 `suspected_player` 角色，checkbox 不出現）、已淘汰／未收編／稽核三區（這張卡都是 0 筆）、第 15–17 項（需要 live AI）。

附帶發現（與本案無關，後端計數）：套用完的訊息說「新增 34 條世界書條目」，實際落地 33 條。前端只是顯示 `refactor_apply` 回傳的 `summary.new_entries`，這段拆檔前後逐字一致，後端本案零改動。已另立案 [refactor-apply-count-mismatch](../tasks/refactor-apply-count-mismatch.md)。
