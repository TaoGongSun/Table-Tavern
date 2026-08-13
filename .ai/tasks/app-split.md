# Task
Task-ID: app-split
Title: App.tsx 拆分：三批 15 切片（元件搬移→controller hook→受控 view）
Status: in-progress

## Summary
App.tsx 6890 行（佔前端 9438 行的七成）拆成多支檔案。目的是維修與管理，不追求任何運行時效益，拆分過程不改行為。目標尺度：一支檔案 200–800 行，App.tsx 收到 400–600 行（依據：專案除 App.tsx 外最大 583 行）。

方案經 Claude（Opus 5）與 Sol（gpt-5.6-sol，Codex CLI）四輪往返達成共識，2026-08-13 使用者拍板三批全做。規格細節（三批範圍、五個 controller 的 contract、五條硬約束、共用符號歸屬、16 項回歸清單與切片對照表）見 [plans/app-split.md](../plans/app-split.md)。

15 個切片：1 前置（check-i18n 掃描範圍）／2–5 第一批元件搬移（CardEditor+CropDialog、WorldEditor、SettingsWindow+UsageTab、atoms）／6–11 第二批 controller（cardInterface、tableState、characters、chat、imports、GenerateTableDialog）／12–15 第三批受控 view（TableSidebar、WorkspaceHeader、PlayView、MainView）。

進度量測：`node scripts/app-structure.mjs` 重跑即可。立案基準＝App() 2908 行、59 state／13 ref／105 區塊。

## Progress
- **第一批（切片 1–5）完成，App.tsx 6890→3241 行。** 1（66a91ed）check-i18n 改遞迴掃 src 下所有 .tsx（實測證明非做不可：CardEditor 搬走後舊版會靜默掉到 86 顆按鈕）；2 CardEditor 780；3 WorldEditor 1351＋外抽 drag-reorder.ts／backend-contracts.ts；4 SettingsWindow 265＋SettingsForm 609＋UsageTab 271／atoms.tsx／model-catalog-store.ts；5 StoryText／EditPane／ActReader 進 atoms.tsx。
- **第二批（切片 6–11）完成，App.tsx 3241→2044，App() 2908→1885 行／59→21 state／105→56 區塊。** 6 useCardInterfaceController 233；7 useTableStateController 187（treeValueAt／loadBranchBindings 提到 module 級並 export）；8 useCharacterController 363；9 useChatController 464；10 useImportController 457；11 GenerateTableDialog 298。
- 第二批的關鍵接線（與規格 contract 表不同，勿動）：imports 掛最後（cardInterface 之後）、tellAboutInterface 進 controller；chattedSinceImport／noteChatRequest／GM_TARGET／speaker／undoLastImport／postOpening／postTranslatedOpening／adoptImportName／openTableForImport 留在 App；chat.postOpening 回傳 boolean，面板關閉由 App 包一層。切片 8 落實硬約束 4：enterTable 三支 await load* 改由 controller 的 effect 延後載入，同步提交區至今零 await。
- **第三批（切片 12–13）完成，App.tsx 2044→1573，App() 1885→1428 行／21→18 state／56→49 區塊。**
- 切片 12（e2ad977）views/TableSidebar.tsx 405：桌次清單＋角色側欄 270 行 JSX 與寬度把手，以 fragment 回傳 aside＋sidebar-resizer 兩兄弟節點。搬進去：sidebarWidth／tableListOpen 兩個 localStorage state 與常數、resize 監聽、castDrag、importInputRef、tierLabel、gm-book。**editingName 留在 App**——它不是純側欄狀態，主欄標題（at:"header"）是同一份改名入口，側欄改吃 renamingTable 布林＋renameForm(className) render prop。WorldMeta 移進 backend-contracts.ts。
- 切片 13（3416a6d）views/WorkspaceHeader.tsx 302，同檔兩個 export：WorkspaceHeader（11 props，桌名＋四顆鈕）與 StateBar（15 props，狀態列＋四支狀態樹 helper 131 行＋stateFields／stateValue／USER_MACRO）。分兩個元件是因為 prop 集完全不相交。stateBarOpen 與 STATE_BAR_OPEN_KEY 搬進去（卸載後靠 localStorage 讀回，行為一致）；hasStateBar 與外層顯示條件留在 App。必要區域改名：helper 參數 tree→isTree、區域 editing→isEditing（原名被 props 佔用）。
- 行為對照（第三批同用逐行 diff）：切片 12 搬走的 270 行換名後只剩 4 處 prettier 折行與一個多餘 void；切片 13 的 header、state-bar、四支 helper、stateFields、USER_MACRO 五段換名後**逐行完全相同**。
- 十三切片驗證：vitest 126／tsc／build／check:i18n 99 顆（10 語系）全綠。
- **第一批完整 16 項回歸（2026-08-13）**：通過＝1、2、3、4、10 開關半、11、13 前半、14、15、16。
- **切片 6–9 針對回歸（2026-08-13）**：6／7 跑第 10、16、9 項；8 跑第 15、2、16 項；9 跑第 14、8 項。每次都含 A→B→A。
- **第二批完成後的完整 16 項回歸（2026-08-13）**：機械項 1、2、3、4、8、9、11、13 前半、14、15、16 全過；5 只驗面板、10 只驗自動開與 ✕ 關、12 只驗對話框。兩張測試桌已刪，26 桌零改動，主題與 last_world 都復原。
- **切片 12／13 針對回歸（2026-08-13，打當前碼的 release 包、自建兩張測試桌）**：第 1 項 A→B→A 全過（狀態列、角色、訊息、導航、卡片介面鈕都正確，無舊桌殘留）；第 9 項平欄（時間→黃昏）與樹葉（Affection 0→42）都改得動、分支指認下拉綁得上；第 15 項建卡→存檔→側欄描邊→編輯→隱藏→隱藏區→還原→刪除全過。順帶通過：側欄列改名（header 標籤同步）、開新的一桌、空桌回收、刪當前桌自動跳最後活動那桌、匯入身分框→匯入成角色卡→桌自動改名→100 條收據、開場白面板貼出這條、卡片介面自動開與 ✕ 關、換幕鈕依 hasEvents 開關、復原上次匯入鈕依 canUndoImport 進出。測試桌全刪、26 桌今天零改動、last_world 與主題都復原。
- **留給使用者的（花額度）**：6（聊天一輪）、7（換幕／分岔／退回／重生摘要）、5 的翻譯語感、10 的殼內送出、12 的 AI 生成。
- **仍未驗的子項**：9 的計數器與分支綁定實際套用、13 後半與 14 的「無桌時補範例桌」、15 的裁圖與未儲存離開 guard、11 的贊助主題鎖。
- **三個既有問題（非拆分造成，已各自立案）**：ActReader「從這一幕繼續」不渲染（act-fork-missing）、換桌不攔未儲存的角色卡（leave-guard-switch-table）、卡片介面覆蓋層 Esc 關不掉（推斷是焦點在沙盒 iframe，宿主收不到 keydown）。

## Next action
切片 14：把 messages＋composer 那段 JSX（約 315 行）抽成 `views/PlayView.tsx` 受控元件，收尾跑三綠＋逐行 diff＋回歸第 6、7、1 項與 A→B→A。

## Constraints
- 一個對話做 1–2 個切片；每個對話結束要產出下一棒的起手提示詞。
- 每切片收尾：`npm test`＋`npm run build`＋`npm run check:i18n` 三綠，加該切片針對案例與固定的 A→B→A 換桌檢查。check:i18n 按鈕數只允許持平或上升（目前 99）。
- 第一批用整段逐字 diff；第二／三批用行為對照——把搬走的定義與搬移前 commit 的對應行逐行 diff，確認差異只有換名，並確認依賴陣列與 hook 宣告順序沒變。換名時小心 replace_all 打到 i18n 字串鍵。
- 每批完成跑完整 16 項手動回歸（清單在 plans/app-split.md）。第三批完成後再跑一次。
- 手動回歸分工＝混合：機械項由 Claude 用 computer-use 驅動；需人眼判品質或花額度的（5、6、7、12）由使用者跑。回歸一律在自建測試桌上做，做完刪掉。
- **要跑實機回歸前，先用 AskUserQuestion 問使用者在不在**（通知聲會把人叫回來按允許），再打包與 request_access。
- **computer-use 看不到 `npm run tauri dev` 的視窗**（裸執行檔沒有 .app bundle）。跑機械回歸前一定要先 `npm run tauri build`，再用 `ps -Ao pid,lstart,command | grep table-tavern` 確認畫面上的進程就是剛打的包。
- 零元件測試（8 支測試全是純函式）是本任務最大風險，手動回歸清單是唯一安全網。
- App.tsx 最終落在 440–570 行：桌次生命週期 9 支 handler 含 enterTable 約 166 行必須留在 composition root。
- commit 以切片為單位，訊息格式 `app-split: 做了什麼（驗證結果）`。
