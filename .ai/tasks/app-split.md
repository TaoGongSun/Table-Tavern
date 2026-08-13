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
- **第二批進行中（切片 6、7、8、9 完成）。App.tsx 3241→2563，App() 2908→2322 行／59→37 state／105→70 區塊。**
- 切片 6（0d68393）：`controllers/useCardInterfaceController.ts`（233）。搬走 cardInterfaces／refactorShell／cardUiOpen、殼 memo 群、兩支載入 effect、message／Esc effect、shellFingerprint／readCardStorage／writeCardStorage，三支函式改 useCallback。hook 掛在原 useEffect@671 的位置，全 App effect 宣告順序不變。3241→3085。
- 切片 7（74c2656）：`controllers/useTableStateController.ts`（187）。搬走 tableState／tableTree／tableJumps／branchBindings／editingStateField＋兩支旗標 ref，四支函式改 useCallback，treeValueAt／loadBranchBindings 提到 module 級並 export，新增 hydrate／beginEdit／changeEditValue／cancelEdit／clearEdit。此域無 useEffect。後端契約 StateNode／SceneLabel／WorldState／BranchBinding 進 `backend-contracts.ts`。3085→2966。
- 切片 8（41cb387）：`controllers/useCharacterController.ts`（363）。搬走 characters／sceneAppearances／playerCard／characterImages／characterAvatars／playerImage／playerAvatar／gmImage＋三支載入函式＋reorderCast／restoreCharacter／restoreAutoHidden／deleteCharacter／deletePlayerCard／refreshCharacters／metaOf／active／archived。2966→2829。
  - **硬約束 4 落地**：enterTable 三支 `await load*` 改由 controller 三支 effect 延後載入（deps 各為 `[worldId, 排序後的 id 集合]`／`[worldId]`／`[worldId, playerCardId]`），`hydrate` 同步清掉上一桌的圖與玩家卡，換桌不再閃舊圖。`scene_appearances` 與 `list_import_receipts` 的 await 一併提到同步區之前，同步提交區現在完全沒有 await。
  - speaker 留在 App：controller 只給 `noteRemoved(id, speaker)` 回報「該撥給誰」。
  - 換名：characters→characters.list／.active／.archived／.images／.avatars／.player／.metaOf／.refresh()／.reorder／.restore／.reloadGmImage／.reloadPlayer／.reloadImages／.onArrived；controller 內 setError→onError、`worldId: table`→`worldId`。
- 切片 9（6a0fc3e）：`controllers/useChatController.ts`（464）。搬走 events／undone／undoBusy／input／generating／streamText／generatingRef／lastTurnAt／pingCount／keepaliveOff／awayTooLong＋appendEvent／postOpening／undoLast／restoreUndone／noteTurnDone／keepalive effect／replyOnce／requestReply／narrateOnce／gmNarrate／gmAdvance／replyFromTarget／send／submitText／canRestore，以及 PLAYER_SENTINEL／KEEPALIVE_*／nowTs。2829→2563。
  - hook 掛在開桌 effect 與 bottomRef effect 之間（cardInterface 之前，那支要吃 chat.submitText）；App 自己剩下的 9 支 useEffect 宣告順序不變。
  - 換名（controller 內）：table→worldId、setError→onError、characters.metaOf→metaOf、characters.player?.name→playerName、characters.active.length→castCount、characters.onArrived→onArrived、tableState.refresh→refreshState、noteChatRequest→noteChatStarted、markCliConnectedFromChat→markCliConnected、setOpeningChoice(null)→closeOpeningChoice()、setWorlds(await list_worlds)→refreshWorlds()、undone.table→undone.worldId。
  - 換名（App 端）：events／generating／streamText／input／awayTooLong／canRestore→chat.*；`generating !== null`→chat.busy；setEvents(transcript)→chat.hydrate；undoLastImport 的 setEvents(await read_transcript)→chat.reload()；advanceScene／regenerateSummary 的 setGenerating+setStreamText 對→chat.beginNarration()／chat.endNarration()。
  - App 端 markCliConnectedFromChat 與 noteChatRequest 改 useCallback（本體逐字相同，後者移到 chat 掛載點之前），新增 refreshWorlds／closeOpeningChoice，gmTargeted 提前；submitText 提到 send 之前、canRestore 提到 restoreUndone 之前（useCallback 有 TDZ）。requestReply 不對外公開（唯一使用者是 controller 內部的 replyFromTarget）。
- 行為對照（第二批用逐行 diff 而非整段逐字）：切片 6／7 的搬移區塊與前一 commit 對應行在換名後 diff 為空；切片 8 差異只有換名與世代號檢查三行；切片 9 與 50de5ae:src/App.tsx 1413-1664 換名後逐行 diff，差異只有 useCallback 包裝、上述換名、keepalive effect 多一行 latest-ref 說明註解，與 send／submitText、canRestore 的宣告位置。
- 四切片驗證：vitest 126／tsc／build／check:i18n 99 顆（10 語系）全綠。
- **第一批完整 16 項回歸（2026-08-13）**：通過＝1、2、3、4、10 開關半、11、13 前半、14、15、16。三張測試桌全刪，磁碟確認 26 桌零改動。
- **切片 6／7 針對回歸（2026-08-13）**：第 10、16、9 項與 A→B→A 全通過。
- **切片 8 針對回歸（2026-08-13，打當前碼的 release 包、自建測試桌）**：第 15、2、16 項與 A→B→A 全通過。兩張測試桌已刪，26 桌零改動，last_world 復原成 Furry World。
- **切片 9 針對回歸（2026-08-13，打當前碼的 release 包、自建兩張測試桌）**：第 14 項通過（改名 header 與側欄同步、A↔B 切換、刪非當前桌不影響當前桌、刪當前桌自動跳到最後活動那桌，確認框都帶對名字）；第 8 項通過（匯入卡的開場白貼上檯面→收回→訊息消失且「復原剛收回的」出現→復原→訊息回來且該鈕消失，收回／換幕鈕的 enabled 狀態全程正確）；A→B→A 通過（訊息／角色／發言對象／卡片介面鈕／復原上次匯入鈕全部隨桌切換，零殘留）；第 6、7 項只驗到按鈕狀態（空桌 disabled、有訊息有角色後 enabled），實際送出與換幕花額度留給使用者。順帶通過第 2 項（匯入角色卡＋38 條世界書）、第 5 項的面板部分（開得出來、展得開、貼得出、貼完自動關）、第 10 項的開關半（匯入後卡片介面自動打開、✕ 關得掉）。兩張測試桌已刪，26 桌今天零改動（僅使用者自己玩過的兩桌），last_world 復原成 Furry World。
- **非迴歸的觀察**：Furry World 那桌「GM 推進」是 disabled，測試桌匯入角色後立刻 enabled——條件 `characters.active.length === 0` 拆分前後逐字相同，該桌的角色應是 auto_hidden 未出場。
- **留給使用者的**：5（開場白翻譯語感）、6（聊天一輪）、7（換幕／分岔／退回／重生摘要）、8（收回訊息＋復原）、9 的計數器與分支綁定實際套用、10 的殼內按鈕送出、12（AI 生成新桌／世界書 AI 重構，已欠三輪）、13 後半與 14 的「無桌時補範例桌」。
- **三個既有問題（非拆分造成，等使用者決定要不要另立案）**：(a) ActReader 的「從這一幕繼續」按鈕在 app 裡不渲染——JSX 與 App.css 從拆分起點 591a2e0 至今零差異；(b) 編輯角色卡有未儲存變更時從側欄換桌不會攔，`canLeaveEditor()` 拆分前後都只掛在 editCard／openPlayerCard／openWorldEditor 三處；(c) 卡片介面覆蓋層開著時按 Esc 關不掉（切片 6 實測，切片 8 再次確認要用 ✕ 才關得掉）。(c) 推斷是鍵盤焦點落在沙盒 iframe、宿主 window 收不到 keydown，非本次拆分造成。

## Next action
切片 10：把 importChoice／importRoute／importReceipts／chattedSinceImport／17 支匯入流程／開場白選擇（openingChoice／openingExpanded／openingTransState／openingTransAllBusy／openingTransAbort）抽成 `src/controllers/useImportController.ts`（注入 characters.refresh／tableState.refresh／cardInterface.refresh／refreshWorlds／enterTable／chat.reload／onError），收尾跑三綠＋逐行 diff＋回歸第 2、3、4、5、16 項與 A→B→A（第 5 項的翻譯花額度，只驗面板開得出來、選得到、關得掉）。

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
