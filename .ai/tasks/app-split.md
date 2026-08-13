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
- **第一批（切片 1–5）全部完成，App.tsx 6890→3241 行。**
- 切片 1（66a91ed）：`scripts/check-i18n.mjs` 改遞迴掃 src 下所有 .tsx。實測證明非做不可——CardEditor 搬走後舊版只掃 App.tsx 會掉到 86 顆按鈕（13 顆靜默漏掉）。
- 切片 2（488e854）：CardEditor＋CropDialog → `views/CardEditor.tsx`（780）。6890→6046。
- 切片 3（8280a35）：WorldEditor → `views/WorldEditor.tsx`（1351，規格表已預先接受超出 800）。外抽 `drag-reorder.ts`、`backend-contracts.ts` 增 Visibility／WorldbookEntry。6046→4630。
- 切片 4（50fb5d1）：使用者拍板拆兩支——`views/SettingsWindow.tsx`（265）＋`views/SettingsForm.tsx`（609）；另出 `views/UsageTab.tsx`（271）、`views/atoms.tsx`（15，ErrorNote）、`model-catalog-store.ts`（70）。`appearance.ts`／`cli.ts` 各有增補。4630→3368。
- 切片 5（ba27149）：StoryText／EditPane／ActReader → `views/atoms.tsx`（15→131）。**`narrationStreamText` 經 grep 確認只在 App() 內用**；跨界的是 `TranscriptEvent` → `backend-contracts.ts`。3368→3241。
- **第二批進行中（切片 6、7、8 完成）。App.tsx 3241→2829，App() 2908→2577 行／59→43 state／105→82 區塊。**
- 切片 6（0d68393）：`controllers/useCardInterfaceController.ts`（233）。搬走 cardInterfaces／refactorShell／cardUiOpen、殼 memo 群、兩支載入 effect、message／Esc effect、shellFingerprint／readCardStorage／writeCardStorage，三支函式改 useCallback。hook 掛在原 useEffect@671 的位置，全 App effect 宣告順序不變。3241→3085。
- 切片 7（74c2656）：`controllers/useTableStateController.ts`（187）。搬走 tableState／tableTree／tableJumps／branchBindings／editingStateField＋兩支旗標 ref，四支函式改 useCallback，treeValueAt／loadBranchBindings 提到 module 級並 export，新增 hydrate／beginEdit／changeEditValue／cancelEdit／clearEdit。此域無 useEffect。後端契約 StateNode／SceneLabel／WorldState／BranchBinding 進 `backend-contracts.ts`。3085→2966。
- 切片 8：`controllers/useCharacterController.ts`（363）。搬走 characters／sceneAppearances／playerCard／characterImages／characterAvatars／playerImage／playerAvatar／gmImage＋三支載入函式＋reorderCast／restoreCharacter／restoreAutoHidden／deleteCharacter／deletePlayerCard／refreshCharacters／metaOf／active／archived。2966→2829。
  - **硬約束 4 落地**：enterTable 三支 `await load*` 改由 controller 三支 effect 延後載入（deps 各為 `[worldId, 排序後的 id 集合]`／`[worldId]`／`[worldId, playerCardId]`），`hydrate` 同步清掉上一桌的圖與玩家卡，換桌不再閃舊圖。`scene_appearances` 與 `list_import_receipts` 的 await 一併提到同步區之前，同步提交區現在完全沒有 await。
  - 新增 `playerCardId` state（玩家卡載入 effect 的依據）與三個世代號 ref（換桌／重讀時丟掉舊回應，避免慢回來的舊桌圖蓋掉新桌）。
  - speaker 留在 App：controller 只給 `noteRemoved(id, speaker)` 回報「該撥給誰」；`remove`／`removePlayer` 只做確認框＋刪檔，關畫面與撥發言對象仍在 App 的 finishRemoval／deleteCharacter／deletePlayerCard。
  - 換名：characters→characters.list、activeCharacters→.active、archivedCharacters→.archived、characterImages→.images、characterAvatars→.avatars、playerCard→.player、playerImage／playerAvatar／gmImage→同名屬性、metaOf→.metaOf、refreshCharacters()→.refresh()、reorderCast→.reorder、restoreCharacter→.restore、loadGmImage(x)→.reloadGmImage(x?)、loadPlayerCard(table,id)→.reloadPlayer(id)、loadCharacterImages(table,characters)→.reloadImages()、setSceneAppearances(...arrived)→.onArrived()；controller 內 setError→onError、`worldId: table`→`worldId`。`loadCharacterImages` 簽名由 `(worldId, cast)` 改 `(worldId, ids)`（本體只用得到 id）。
- 行為對照（第二批用逐行 diff 而非整段逐字）：切片 6／7 的搬移區塊與前一 commit 對應行在換名後 diff 為空；切片 8 的三支載入函式、reorder、restore／restoreAutoHidden、remove、removePlayer 與 74c2656 對應行逐行 diff 亦為空，差異只有上面那批換名與新增的世代號檢查三行。App 自己的 10 支 useEffect 宣告順序三個切片後完全不變（`node scripts/app-structure.mjs` 對照）。
- 三切片驗證：vitest 126／tsc／build／check:i18n 99 顆（10 語系）全綠。
- **第一批完整 16 項回歸（2026-08-13）**：通過＝1、2、3、4、10 開關半、11、13 前半、14、15、16。三張測試桌全刪，磁碟確認 26 桌零改動。
- **切片 6／7 針對回歸（2026-08-13）**：第 10、16、9 項與 A→B→A 全通過。
- **切片 8 針對回歸（2026-08-13，打當前碼的 release 包、自建測試桌）**：第 15 項通過（建卡→裁圖→存檔→側欄縮圖更新→封存→隱藏區還原→建玩家卡→刪玩家卡→刪角色卡，確認框都帶對名字，未儲存離開 guard 有攔且 Cancel 留在編輯器）；第 2 項通過（匯入角色卡直匯這桌，再匯第二張走路由框選「開新桌並匯入」，新桌名／角色／狀態列／卡片介面都正確）；第 16 項通過（復原後角色卡收回、發言對象撥回 GM、卡片介面鈕與復原鈕一起消失、GM 圖回到書本圖）；A→B→A 通過（兩桌各有角色圖，來回切換零殘留，狀態列／狀態樹隨桌正確出現與消失）。兩張測試桌已刪，26 桌今天零改動（僅使用者自己 03:05／09:04 玩過的兩桌），last_world 復原成 Furry World。
- **留給使用者的**：5（開場白翻譯語感）、6（聊天一輪）、7（換幕／分岔／退回／重生摘要）、8（收回訊息＋復原）、9 的計數器與分支綁定實際套用、10 的殼內按鈕送出、12（AI 生成新桌／世界書 AI 重構，已欠三輪）、13 後半與 14 的「無桌時補範例桌」。
- **三個既有問題（非拆分造成，等使用者決定要不要另立案）**：(a) ActReader 的「從這一幕繼續」按鈕在 app 裡不渲染——JSX 與 App.css 從拆分起點 591a2e0 至今零差異；(b) 編輯角色卡有未儲存變更時從側欄換桌不會攔，`canLeaveEditor()` 拆分前後都只掛在 editCard／openPlayerCard／openWorldEditor 三處；(c) 卡片介面覆蓋層開著時按 Esc 關不掉（切片 6 實測，切片 8 再次確認要用 ✕ 才關得掉）。(c) 推斷是鍵盤焦點落在沙盒 iframe、宿主 window 收不到 keydown，非本次拆分造成。

## Next action
切片 9：把 events／undone／generating／streamText／input／undo 與 keepalive ref／對話 handler 抽成 `src/controllers/useChatController.ts`（generating 對外公開為 chat.busy 給桌次操作讀，注入 tableState.refresh／refreshWorlds／imports.noteChatStarted／characters.onArrived／onError），收尾跑三綠＋逐行 diff＋回歸第 6、7、8、14 項（花額度的部分留給使用者）與 A→B→A。

## Constraints
- 一個對話做 1–2 個切片；每個對話結束要產出下一棒的起手提示詞。
- 每切片收尾：`npm test`＋`npm run build`＋`npm run check:i18n` 三綠，加該切片針對案例與固定的 A→B→A 換桌檢查。check:i18n 按鈕數只允許持平或上升（目前 99）。
- 第一批用整段逐字 diff；第二批改行為對照——把搬走的定義與搬移前 commit 的對應行逐行 diff，確認差異只有換名，並確認依賴陣列與 hook 宣告順序沒變。
- 每批完成跑完整 16 項手動回歸（清單在 plans/app-split.md）。第二批六個 controller 全完成後、第三批完成後各再跑一次。
- 手動回歸分工＝混合：機械項由 Claude 用 computer-use 驅動；需人眼判品質或花額度的（5、6、7、12，以及任何要 AI 產出的子項）由使用者跑。回歸一律在自建測試桌上做，做完刪掉。
- **computer-use 看不到 `npm run tauri dev` 的視窗**（裸執行檔沒有 .app bundle，畫面過濾只認 bundle id），而且 request_access 會把舊的 release 包叫起來、讓人誤以為在測新碼。跑機械回歸前一定要先 `npm run tauri build`（約 3 分鐘），再用 `ps -Ao pid,lstart,command | grep table-tavern` 確認畫面上的進程就是剛打的包。
- 零元件測試（8 支測試全是純函式）是本任務最大風險，手動回歸清單是唯一安全網。
- App.tsx 最終落在 440–570 行：桌次生命週期 9 支 handler 含 enterTable 約 166 行必須留在 composition root。
- commit 以切片為單位，訊息格式 `app-split: 做了什麼（驗證結果）`。
