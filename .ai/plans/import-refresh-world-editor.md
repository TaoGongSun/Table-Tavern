# import-refresh-world-editor 方案

> 結案 2026-10-11：已進 main，三方驗收通過。

## 現況
- 會寫進世界書的匯入入口只有一個：陣容欄「匯入卡」（`CastRail` → `imports.importFile`；前端沒有拖放）。分流後寫世界書的路：
  - `importAsCharacter`（角色卡＋隨身世界書，`import_character`）與 `importAsWorldbook`（世界書檔／世界書卡，`import_worldbook`）：直接匯、身分框作答、路由框選「匯進這桌」都落到這兩支，寫的是目前這桌。
  - 路由框「開新桌並匯入」、網頁存檔：開新桌前過 `canLeaveRef`、進桌時 `setMainView(null)`，世界設定已關〔模型判斷·未裁決〕。
  - 重構卡 PNG：不在本案範圍〔模型判斷·未裁決〕。它的 `refreshAfterApply` 只重讀清單，不處理開著的既有條目草稿，整條改寫可能被自動存蓋回——主線另立案。
- 兩條匯入完成後都不通知世界設定：條目清單、勾選名單、帳本都不重讀，要關掉重開才看到。
- 匯入對既有條目只會改一件事：內容重複的來源條目併進桌上那條時，`merge_entry_into` 擴大可見度並合併來源卡（`BookImport.restores` 已逐條記下）。草稿沒有來源卡欄，只有可見度會跟開著的草稿衝突：開著那條、改了任一欄後離開，自動存會用舊可見度蓋回，新角色看不到那條，無聲。

## 做法

### 1. 後端回報擴大了可見度的既有條目
- `import::Imported` 加 `widened_entries: Vec<u64>`：在 `restores` 移進 `record_*_import` 前，取 `after_visibility != before_visibility` 的 uid（只動來源卡的不算）。角色卡路、世界書路都填；沒書或書匯失敗是空。
- `CharacterImport`、`WorldbookImportResult` 各加同名欄位照傳。網頁存檔不動〔模型判斷·未裁決〕。

### 2. 匯入完成發世界設定通知（沿用 WorldChange）
- `useImportController` 加注入 `onWorldbookChanged(worldId, widened)`；`importAsCharacter`／`importAsWorldbook` 在後端回來後第一步呼叫（後續步驟失敗也已通知）；後端丟錯不呼叫。前端讀 `widened_entries ?? []`。
- navigation 加 `noteImported(worldId, widened)`：一律用傳入的 worldId 發——開新桌並匯入時 controller 拿到的是舊閉包，不能用閉包裡的桌 id。`postWorldChange` 改成吃 worldId 參數。前提：換桌與匯入同受 `runTableOp` 互斥，傳入的桌必是匯入當下的桌；拿掉這把鎖時要重看這裡。發的內容：`ids` 空；`widened` 非空時帶一筆 `{ changed: widened.map(uid => ({ uid, groups: ["visibility"] })), deleted: [] }`。
- 世界設定收到後走既有 `applyWorldChange`：重讀條目、勾選名單、帳本；新條目草稿、world.md、沒被擴大的既有條目草稿都不動。沒開時照既有語意：累積、掛載時取走不套用。

### 3. 開著的既有條目被擴大
- 走撤銷已定案的待定基：該條草稿的可見度組與原樣快照換成重讀值，其他欄組照留；定基前擋寫回、重讀失敗不寫（undo-import-editor-guard 第 3 節）。玩家在這條草稿裡改過的可見度以匯入值為準〔模型判斷·未裁決，依作者 2026-10-11 罕見輕防原則〕。
- 通知欄位與型別改名：`WorldChange.undoChanges` → `entryChanges`、`UndoEntryChanges` → `EntryChanges`（後端撤銷的 `entry_changes` 不動）。

### 4. App.tsx 不拆〔模型判斷·未裁決〕
撤銷善後是跨域協調，不屬 `features/import`，純重構也與本案無關；App.tsx 超 1000 行照規約可接受。

## 掛點
- 後端：`src-tauri/src/import/files.rs`、`commands/character.rs`、`commands/world.rs`。
- 前端：`features/import/useImportController.ts`、`App.tsx`（接注入、`UndoReport` 型別改名）、`controllers/useWorkspaceNavigationController.ts`、`features/worldbook/worldbook-model.ts`／`useWorldbookEditor.ts`／`WorldEditor.tsx`（改名與註解）。
- 測試改名跟著動：`removal-navigation.test.ts`、`WorldEditor.cast-removal.test.tsx`、`WorldEditor.test.tsx`、`WorldEditor.undo-import.test.tsx`；必要時補 mock（image-dropped-notice／opening-source／app-navigation／web-save-routing）。

## 測試
- Rust：世界書路與角色卡路對等三類——擴大（明寫 public 或含本桌 id 的名單）→ 含該 uid；只動來源卡（沿用 `card_book_tests.rs` 既有資料）→ 不含；全新條目 → 空。同一既有 uid 被多個來源條目合併只報一次。帳本新增收編紀錄斷言。
- navigation：`noteImported` 用傳入桌 id 發（模擬開新桌並匯入的舊閉包）；有 widened → 帶 visibility 組；匯入＋撤銷通知同桌累積，舊世代取走不清新通知。
- controller：通知是後端回來後第一步；後端丟錯不呼叫；後續 `refreshCharacters`／`reloadGmImage` 失敗仍已送出；驗傳入桌 id 與 widened。
- `WorldEditor.import.test.tsx`：清單出現新條目、勾選名單出現新角色、`read_world_md` 只呼叫一次、world.md 未存文字與新條目草稿都在；被擴大的既有草稿改過內文 → 可見度採重讀值、內文留、離開自動存 payload 正確；沒被擴大的既有草稿不動；匯入重讀未完成／失敗時觸發自動存與帳本開關 → 零過期 payload。
- App 層整合（比照 `undo-import-editor.test.tsx`）：世界設定開著經陣容欄匯入 → 條目重讀、world.md 只讀一次、未存文字還在。
- `npm run verify` 12 步全綠。

## 驗收：測試通道零 AI
`npm run harness:build` → `node scripts/harness.mjs launch --fresh --root <scratch>`；scratch 自造兩張 JSON 角色卡 X、Y，各帶專屬條目，並共用一條會擴大可見度的條目。
1. 匯入 X。開世界設定，改 world.md 不存、開一條新條目草稿；匯入 Y（路由框答「匯進這桌」）→ 世界設定仍開著、Y 的專屬條目出現、勾選名單有 Y、world.md 與新條目草稿都在。
2. 撤銷 Y；開共用條目改內文不存 → 再匯入 Y → 可見度含 Y、內文改動還在；切條目自動存後檔案內容相符。
3. 世界設定開著時匯入一本純世界書 JSON → 新條目出現。
每步截圖存 scratch。

## 已知限制（罕見不嚴防〔模型判斷·未裁決〕）
- 從匯入送出到世界設定收到通知之間送出的自動存、手動儲存、帳本 upsert，都可能用舊可見度蓋回擴大結果。
- 玩家在被擴大那條草稿裡改過的可見度，被匯入值取代。
- 世界書路寫完條目後 `save_gm_image` 失敗：命令回錯，不發通知（不在錯誤分支補發）。
