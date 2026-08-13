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
- **第一批（切片 1–5）完成，App.tsx 6890→3241 行。** 1（66a91ed）check-i18n 改遞迴掃 src 下所有 .tsx（實測證明非做不可：CardEditor 搬走後舊版會靜默掉到 86 顆按鈕）；2（488e854）views/CardEditor.tsx 780；3（8280a35）views/WorldEditor.tsx 1351＋外抽 drag-reorder.ts／backend-contracts.ts；4（50fb5d1）使用者拍板拆 SettingsWindow.tsx 265＋SettingsForm.tsx 609，另出 UsageTab.tsx 271／atoms.tsx／model-catalog-store.ts；5（ba27149）StoryText／EditPane／ActReader 進 atoms.tsx（131）。
- **第二批（切片 6–11）全部完成，App.tsx 3241→2044，App() 2908→1885 行／59→21 state／105→56 區塊。**
- 切片 6（0d68393）useCardInterfaceController 233；7（74c2656）useTableStateController 187（treeValueAt／loadBranchBindings 提到 module 級並 export）；8（41cb387）useCharacterController 363；9（6a0fc3e）useChatController 464。
- 切片 8 落實硬約束 4：enterTable 三支 `await load*` 改由 controller 的 effect 延後載入，hydrate 同步清掉上一桌的圖，換桌不再閃舊圖；同步提交區至今零 await。speaker 與 GM_TARGET 留在 App。
- 切片 10（903d25b）useImportController 457：匯入身分框、第二張卡路由、匯入收據與匯完的開場白面板整組搬走。**接線與規格 contract 表不同**——規格表隱含 chat→imports→cardInterface→chat 的環，斷在 chat→imports：imports 掛最後（cardInterface 之後）、tellAboutInterface 跟著搬進 controller；chattedSinceImport 與 noteChatRequest 留在 App（controller 用注入的 resetChatted 清記號）；chat.postOpening 改回傳 boolean，面板關閉交給呼叫端的 App 包一層（貼失敗不關，與原本等價）；openNewTableAndImport 的開桌那一半留在 App 當 openTableForImport，回傳新桌 id。undoLastImport／postTranslatedOpening／worldEditorRefreshKey／importInputRef 留在 App。adoptImportName 改 useCallback 並移到 hook 之前。
- 切片 11（d51f5a4）views/GenerateTableDialog.tsx 298：9 個 gen* state＋三支生成流程＋138 行 JSX 整組搬走，GENRE_KEYS 與四個 gen 型別跟著走。開關留 App，用 `open` prop＋元件內 `if (!open) return null`（不用 `{open && <Dialog/>}`，那會卸載元件把草稿清掉）；注入收斂成 onClose 與 onCreated(worldId)。
- 行為對照（第二批用逐行 diff）：六個切片搬走的定義對前一 commit 逐行 diff，差異只有換名、useCallback 包裝與依賴陣列；切片 10 的三個 modal JSX、切片 11 的整段 JSX 換名後逐行相同。
- 六切片驗證：vitest 126／tsc／build／check:i18n 99 顆（10 語系）全綠。
- **第一批完整 16 項回歸（2026-08-13）**：通過＝1、2、3、4、10 開關半、11、13 前半、14、15、16。三張測試桌全刪，磁碟確認 26 桌零改動。
- **切片 6–9 針對回歸（2026-08-13）**：6／7 跑第 10、16、9 項；8 跑第 15、2、16 項；9 跑第 14、8 項與第 2、5（面板）、10（開關）順帶通過。每次都含 A→B→A，測試桌事後全刪、last_world 復原。
- **第二批完成後的完整 16 項回歸（2026-08-13，打當前碼的 release 包、自建兩張測試桌）**：機械項 1、2、3、4、8、9、11、13 前半、14、15、16 全過；5 只驗面板（展開→貼出→面板自動關→「先不要」關得掉）、10 只驗自動開與 ✕ 關、12 只驗對話框（類型選得到、關掉再開草稿仍在）。重點確認：開新桌並匯入用卡名建桌並進去、檯面零殘留；世界書匯完發言對象指回 GM；已改名的桌不被第二次匯入改名；復原上次匯入帶對最後一筆並全域刷新；刪當前桌自動跳到最後活動那桌。兩張測試桌已刪，26 桌今天零改動，主題與 last_world（Furry World）都復原，冷啟動回得去。
- **留給使用者的（花額度）**：6（聊天一輪）、7（換幕／分岔／退回／重生摘要）、5 的翻譯語感、10 的殼內送出、12 的 AI 生成。
- **非迴歸的觀察**：Furry World 那桌「GM 推進」是 disabled，測試桌匯入角色後立刻 enabled——條件 `characters.active.length === 0` 拆分前後逐字相同，該桌的角色應是 auto_hidden 未出場。
- **仍未驗的子項**：9 的計數器與分支綁定實際套用、13 後半與 14 的「無桌時補範例桌」、15 的裁圖與未儲存離開 guard、11 的贊助主題鎖。
- **三個既有問題（非拆分造成，已各自立案）**：ActReader「從這一幕繼續」不渲染（act-fork-missing）、換桌不攔未儲存的角色卡（leave-guard-switch-table）、卡片介面覆蓋層 Esc 關不掉（推斷是焦點在沙盒 iframe，宿主收不到 keydown）。

## Next action
切片 12：把桌次＋角色側欄那段 JSX（約 270 行）抽成 `views/TableSidebar.tsx` 受控元件，只吃 controller 的 view model 與 actions，收尾跑三綠＋逐行 diff＋回歸第 1、15 項與 A→B→A。

## Constraints
- 一個對話做 1–2 個切片；每個對話結束要產出下一棒的起手提示詞。
- 每切片收尾：`npm test`＋`npm run build`＋`npm run check:i18n` 三綠，加該切片針對案例與固定的 A→B→A 換桌檢查。check:i18n 按鈕數只允許持平或上升（目前 99）。
- 第一批用整段逐字 diff；第二批改行為對照——把搬走的定義與搬移前 commit 的對應行逐行 diff，確認差異只有換名，並確認依賴陣列與 hook 宣告順序沒變。
- 每批完成跑完整 16 項手動回歸（清單在 plans/app-split.md）。第三批完成後再跑一次。
- 手動回歸分工＝混合：機械項由 Claude 用 computer-use 驅動；需人眼判品質或花額度的（5、6、7、12，以及任何要 AI 產出的子項）由使用者跑。回歸一律在自建測試桌上做，做完刪掉。
- **要跑實機回歸前，先用 AskUserQuestion 問使用者在不在**（通知聲會把人叫回來按允許），再打包與 request_access。
- **computer-use 看不到 `npm run tauri dev` 的視窗**（裸執行檔沒有 .app bundle，畫面過濾只認 bundle id），而且 request_access 會把舊的 release 包叫起來、讓人誤以為在測新碼。跑機械回歸前一定要先 `npm run tauri build`（約 3 分鐘），再用 `ps -Ao pid,lstart,command | grep table-tavern` 確認畫面上的進程就是剛打的包。
- 零元件測試（8 支測試全是純函式）是本任務最大風險，手動回歸清單是唯一安全網。
- App.tsx 最終落在 440–570 行：桌次生命週期 9 支 handler 含 enterTable 約 166 行必須留在 composition root。
- commit 以切片為單位，訊息格式 `app-split: 做了什麼（驗證結果）`。
