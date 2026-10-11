# undo-import-editor-guard 方案

> 結案 2026-10-11：已進 main，三方驗收通過。

## 現況
- `App.runUndoLastImport` 撤銷完用 `setWorldEditorRefreshKey` 改 `MainView` 裡 WorldEditor 的 `key`，整個重掛載：未存的 world.md 與條目草稿靜默消失。開頭不問 `canLeaveEditor`。
- 撤銷刪掉的角色卡若正開在 CardEditor／玩家卡編輯器，畫面不關，停在已刪的卡上。
- `UndoReport` 只回名字（`removed_character`、`removed_characters`），前端拿不到 id；後端 `undo_last` 其實已算出確定已刪的 `gone_characters`（`character_card_gone` 為真者）。
- 撤銷不寫 world.md（收據與匯入都不碰它）。世界書會被改的方式：刪本次新建且指紋未變的條目（2）、還原被合併擴大的可見度與來源卡（1a）、整條覆寫回快照（3）、插回本次刪掉的條目（3a，可能換新 uid）、從名單剪掉已刪角色（3b）；另動帳本、狀態樹、玩家卡指定（`player_card_assigned` 時退回 None）。
- 撤銷一個重構套用的玩家卡後，前端側欄玩家卡不重讀（`list_characters` 不含玩家卡，undo 流程沒呼叫 `reloadPlayer`）。
- 世界書存條目指令遇到不存在的 uid 會以新 uid 新增，過期草稿寫回會讓已刪條目復活。
- 跨桌晚到：`useCharacterController` 的 `remove`／`removePlayer`／`restore`／`restoreAutoHidden`／`reorder` 在 await 後直接 `onError` 或寫 state，換桌後晚到會落在新桌（`reorder` 回捲還會把上一桌名單塞進新桌）。

## 做法

### 1. 後端回報撤銷實際改了什麼（`receipts.rs`）
- `UndoReport` 加：
  - `removed_character_ids: Vec<String>`：填 `gone_characters`（單張、多張兩條都已收集，含刪除回錯但卡檔確定不在的）。前端只拿它決定關誰、剪誰。
  - `entry_changes: Option<EntryChanges>`，`EntryChanges { changed: Vec<{ uid, groups: Vec<String> }>, deleted: Vec<u64> }`。欄組固定為 `title`、`keys`、`content`、`constant`、`enabled`、`order`、`visibility`（可見度＋名單同組；草稿沒有的欄位如來源卡、次要鍵不報）。
- 算法：撤銷持整桌獨占，期間只有撤銷自己寫世界書。步驟 1（刪角色）之前讀一次 `read_worldbook` 前快照，所有世界書步驟做完後再讀一次後快照。`deleted`＝前有後無的 uid ∪ 步驟 2 實際刪掉、之後又被佔用的 uid（一律算刪除，刪除判定優先於欄位差異）。其餘兩邊都有的 uid 逐欄組比：前快照先用 `removed_character_ids` 剪名單（同 `scrub_entry` 規則）再比，純剪 id 的差異不報〔作者裁決 2026-10-11〕，有差的進 `changed`。匯入擴大的可見度被撤銷還原、結果恰等於剪掉被刪卡時，視同純剪 id、不報 visibility〔作者裁決 2026-10-11 延伸〕。3a 插回、前快照沒有的新 uid 不報。
- 前後快照讀取不套 `kept(strict, …)?`：任一次讀失敗只記 `None`，不中斷撤銷，也不影響重構失敗的自動回滾。

### 2. 撤銷完的善後（`App.runUndoLastImport` → navigation）
- 後端回來、`live()` 為真後**第一步**呼叫 navigation 新函式 `finishUndoImport(report)`（先於其他重讀，免得被刪卡的編輯器在名單重讀途中讀卡報錯）：
  - 關卡：沿用 `settleRemoval` 的判斷——當下是四種卡片畫面（含 `player`）且 id 在 `removed_character_ids` 裡才關，實際關了才清 `leaveGuard`；不另問〔作者裁決 2026-10-11：刪的正是編輯中那張不另問，撤銷確認框就是最後一道〕。別張卡、新卡草稿、世界設定、單幕閱讀都不動。
  - 一律發世界設定通知（見 3），帶 `removed_character_ids` 與 `entry_changes`。
  - 進桌世代 `stillHere()` 不符就整段不做。
- 撤銷開頭不加 `canLeaveEditor`：撤銷不再換掉任何未存畫面，唯一會關的是被撤銷的那張卡，照上一條不問。
- 補玩家卡重讀：放在 `imports.refreshReceipts` 之後，自己包 try（失敗只報錯，不擋完成訊息）：`read_state` → `characters.reloadPlayer(state.player_card_id)`。
- 拿掉 `worldEditorRefreshKey` 整條（App state、`AppWorkspace`／`MainView` prop 與 `key`）。
- `App.tsx` 的 `UndoReport` 型別與 `refactor-undo-flow.test.tsx` 兩處 mock 補新欄位。

### 3. 世界設定通知
- `CastRemoval` 改名 `WorldChange`：`{ worldId, generation, removedIds, undoChanges: UndoChange[] }`，`UndoChange` 是一次撤銷的 `entry_changes`（`None` 也照樣放一筆，標成未知）。刪角色只加 `removedIds`，撤銷兩者都加；累積、取走、比對世代與桌的語意不變，取走時兩欄一起清。不另設 `undone` 旗標。
- WorldEditor 掛載時：已有的同桌通知直接取走（呼叫 `onCastRemovalTaken`）、不套用——掛載本來就讀新資料，也不讓撤銷資訊殘留到之後的刪角色通知。
- `useWorldbookEditor.applyWorldChange(removedIds, undoChanges)`：
  1. 照舊用 `removedIds` 修剪草稿與原樣快照（重讀失敗也做）。
  2. `undoChanges` 裡 `None` 的照上面「未知」當下處理；其餘併進 hook 內的**待定基** ref（同 uid 欄組取聯集、`deleted` 取聯集）。`[world]` effect 清空它；取出與清 ref 放在 setState 更新函式外。待定基只在「之後任一次成功且是最新票號的條目重讀」套用後清空；重讀失敗、被較新重讀頂掉的舊回應都不清。套用寫在 `refreshWorldbook` 的成功分支，所以通知自己的重讀、存條目後的重讀、帳本開關後的重讀都能完成它。
  3. 照舊重讀條目、名單、帳本（各自 ticket）。
- 定基（純函式 `rebaseDraft(state, pending, freshEntries)` 放 `worldbook-model.ts`；`fresh` 用跟 `editEntry` 同一支 `draftFromEntry` 轉，產出鍵順序與 `draftFromEntry` 一致，`draftDirty` 仍用 `JSON.stringify` 比）：
  - 新條目草稿（`uid === null`）：不動。
  - 草稿 uid 在 `deleted`：關掉，修改丟棄、不另問〔作者裁決 2026-10-11〕。
  - 草稿 uid 在 `changed`：重讀結果找不到該 uid → 關草稿。找得到 → 只對報上來的欄組，草稿與原樣快照都換成重讀值，玩家在這些欄組的輸入丟掉〔作者裁決 2026-10-11：被撤銷改到的那一欄以撤銷為準〕；其他欄組草稿與快照都不動。之後照舊套 `removedIds` 修剪：撤銷刪角色剪名單造成的可見度變動不算撤銷改欄，快照 X,Y／草稿 X,Y,Z／撤銷刪 Y → 草稿 X,Z，與一般刪角色一致〔作者裁決 2026-10-11〕；撤銷真的改了可見度（例如重構改寫過的條目整條覆寫回去；匯入擴大的可見度還原恰等於剪掉撤銷的卡，不算）才整組採重讀值。不拿「重讀值≠快照」推斷。
  - 未知（後端回 `None`）：收到通知當下就關掉開著的既有條目草稿、不另問；新條目草稿與 world.md 不動，不進待定基、不擋寫回〔模型判斷·未裁決，依作者「罕見不嚴防」原則〕。
  - 都沒命中：不動。
- `draft` 與 `draftOrigin` 合成單一 state（`{ draft, origin }`），修剪與定基各在一次函式式更新裡同時算，重讀途中玩家打的字以套用當下為準。
- `WorldbookSection.tsx` 的捲動 effect 改依「開了哪條」（`openDraft` 遞增的開啟序號）觸發，不再依 `draftOrigin`，定基改快照不會亂捲。
- 待定基期間擋寫回：離開與切條目的自動存（`openDraft`、`flushExistingDraftForLeave`）先看「沒草稿、新條目或草稿乾淨」就直接放行；手動儲存（`saveEntry`）與 `convertEntryToCharacter` 不論乾淨與否一律等；帳本開關也先等定基與進行中的重讀，再從最新清單（ref）取整條，找不到就跳過。等待時：反覆發條目重讀直到待定基 ref 確實清空（不以某次 `refreshWorldbook` 返回為準——票號過期的呼叫會直接返回），清空後從最新 state（ref 鏡像）取當下草稿再決定寫不寫，不沿用 await 前的閉包；草稿已關就照關掉處理（切條目照開新的、離開照放行、不送 upsert）。重讀失敗就不寫、留在原處並顯示錯誤（離開被擋）。
- world.md 的 `text`／`savedText` 不碰（撤銷不寫 world.md）。

### 4. 後端防線：已刪條目不復活
- `upsert_worldbook_entry` 指令加參數 `create: bool`。前端新條目草稿傳 `true`；既有條目草稿與帳本開關傳 `false`。`create: false` 而在提交鎖內找不到該 uid → 回錯 `worldbook_entry_gone`（UiMsg 代碼，十語系），不寫入。放在指令層的 `upsert_entry_dropping_gone`，資料層不動（撤銷插回、重構、匯入合併照舊走資料層）。
- 前端 `persistDraft` 收到 `worldbook_entry_gone`：只關掉送出的那份草稿（以開啟序號比對，等待途中新開的草稿不動）、重讀條目、訊息列顯示「這條已被刪除」；回傳值讓呼叫端照「已關」處理（切條目繼續、離開放行）。

### 5. 重構對話框不再隨撤銷重設
拿掉 `key` 後，WorldEditor 裡的重構流程狀態（結果框、開檔素材、token）撤銷後原樣在。結果框的條目清單吃重讀後的 `entries`；撤銷後仍可套用或取消，素材照原生命週期釋放〔模型判斷·未裁決〕。

### 6. 跨桌晚到（`useCharacterController`）
- 比照 navigation 加進桌世代（`worldId` 變就遞增，含 w1→w2→w1）。
- `remove`：await 後世代不符不呼叫 `onError`（照回結果，navigation 那側有 `stillHere()`）。
- `removePlayer`：await 後世代不符不寫玩家卡 state、不報錯、回 `false`；navigation `deletePlayerCard` 也補 `stillHere()` 才 `setMainView(null)`。
- `restore`／`restoreAutoHidden`：寫入 await 回來先查世代，不符就不呼叫 `refresh()`、不報錯。
- `reorder`：失敗不回捲到舊快照（期間可能有重讀加進新角色），改為報錯並從磁碟重讀名單；換桌後、或之後又排過序（自己的排序序號）就不動。

## 掛點
- 後端：`src-tauri/src/receipts.rs`（`UndoReport` 兩欄、前後快照比對）、`src-tauri/src/commands/world.rs`＋`receipts/character_delete.rs`（`create` 參數與 `worldbook_entry_gone`）、`ui_msg` 與十語系。
- `src/App.tsx`：`UndoReport` 型別、`finishUndoImport`、玩家卡重讀、刪 `worldEditorRefreshKey`。
- `src/controllers/useWorkspaceNavigationController.ts`：`finishUndoImport`、`WorldChange`、`deletePlayerCard` 的 `stillHere()`。
- `src/views/AppWorkspace.tsx`、`src/views/MainView.tsx`：拿掉 `key`，通知改名。
- `src/features/worldbook/WorldEditor.tsx`、`useWorldbookEditor.ts`、`worldbook-model.ts`（`draftFromEntry`、`rebaseDraft`）、`WorldbookSection.tsx`（捲動觸發）。
- `src/features/characters/useCharacterController.ts`。
- `receipts/upsert_guard_tests.rs`：`upsert_entry_dropping_gone` 改簽名後呼叫跟著改。

## 測試
- Rust：
  - `receipts/tests.rs`：`removed_character_ids` 單張、多張、刪除回錯但卡檔已不在 → 含；卡刪不掉仍在 → 不含（權限測試，root 下略過）。
  - `entry_changes`：刪本次條目進 `deleted`；匯入擴大的可見度還原恰等於剪掉撤銷的卡 → 不報；整條覆寫只報實際不同的欄組（含可見度）；撤銷刪角色時 3b 純剪名單不報；3a 插回、uid 被佔走（raw 與非 raw 兩條）→ 該 uid 報 `deleted`；3a 插回的新 uid 不報；世界書讀不了 → `None` 且撤銷照做。
  - 指令層：`create: false` 找不到 uid → `worldbook_entry_gone` 且檔案不變；`create: true` 照舊新增；既有 uid 照舊更新。
- `worldbook-model.test.ts`（`rebaseDraft`）：只換報上來的欄組；沒報的欄組（含與重讀值不同者）保留草稿；快照 X,Y／草稿 X,Y,Z／撤銷刪 Y（沒報 visibility）→ 修剪後 X,Z；報了 visibility → 整組採重讀值；`deleted` 或 `changed` 但重讀找不到 → 關；沒改過的草稿定基後乾淨；鍵順序與 `draftFromEntry` 一致。
- `WorldEditor` 通知測試（cast-removal 檔照改名；撤銷情境另開 `WorldEditor.undo-import.test.tsx`）：
  - 撤銷通知後不重掛載（`read_world_md` 只呼叫一次）、world.md 與新條目草稿都在。
  - 乾淨既有草稿跟上還原的可見度、離開不 upsert；改過內文、撤銷改可見度 → 內文留、可見度採撤銷；同欄衝突採撤銷值；條目被刪（含改過的）→ 關且不跳確認。
  - 世界設定沒開時撤銷 → 打開 → 刪角色：不定基（只剪 id）。
  - 重讀途中又有一次重讀（存條目或帳本開關）→ 仍會定基；舊回應丟掉不清待定基；入口等待用的重讀被帳本重讀頂掉 → 仍等到待定基清空才寫。
  - 重讀途中切換條目 → 等定基完成才決定自動存，被刪的條目零 upsert。
  - 重讀失敗／未完成時離開或切條目 → 零 upsert；之後重讀成功才套用。`None` 通知 → 既有條目草稿當下關、新條目與 world.md 在。`worldbook_entry_gone` 只關送出的那份草稿。
  - upsert 回 `worldbook_entry_gone` → 草稿關、離開放行。
  - 延遲撤銷重讀時按乾淨草稿儲存、帳本開關：等定基後才寫，payload 是撤銷後的整條；轉成角色卡等定基、條目被刪就不轉；切條目遇 `worldbook_entry_gone` 訊息留著。
  - 刪角色通知只剪 id，既有 cast-removal 測試全過。
  - 撤銷後重構結果框仍可套用與取消（素材正常釋放）。
- `removal-navigation.test.ts`：`finishUndoImport` 只關顯示被撤銷 id 的畫面（含 `player`），實際關了才清 guard；別張卡、新卡、世界設定、單幕不動；沒刪角色也發通知；換桌後不動；`deletePlayerCard` 換桌晚到不關畫面。
- App 層（同 `refactor-undo-flow.test.tsx` fixture 另開檔）：開世界設定改 world.md 後撤銷 → 編輯器還在、字還在、`confirm` 只出現撤銷那一次；開著被撤銷的卡 → 關且不另問；開著別張卡 → 不關；撤銷退回玩家卡指定 → 側欄玩家卡清掉；`read_state` 失敗不擋完成訊息。
- `useCharacterController.test.ts`：`remove`、`removePlayer`、`restore` 在換桌（含 w1→w2→w1）後才回來 → 不報錯、不改新桌 state、`removePlayer` 回 false、`restore` 不呼叫 `list_characters`；較早的 `reorder` 失敗晚於較晚排序回來 → 不動；重讀途中排序失敗 → 名單以磁碟重讀為準。
- `npm run verify` 全綠。

## 驗收：測試通道零 AI
`npm run harness:build` → `node scripts/harness.mjs launch --fresh --root <scratch>`；在 scratch 自造兩張 JSON 角色卡 X、Y，內嵌世界書各有專屬條目，並共用一條會在匯入合併時擴大可見度的條目（照 `import/card_book_tests.rs` 的 `undo_restores_only_visibility_of_widened_entries` 做）。
1. 匯入 X、再匯入 Y。開世界設定：改 world.md 不存、開一條新條目草稿；從陣容欄撤銷上次匯入（對話窗指令按確認）→ 世界設定仍開著、world.md 與新條目草稿都在；共用條目可見度退回只有 X；勾選名單沒有 Y。
2. 重新匯入 Y；開共用條目改內文不存 → 撤銷 → 可見度採撤銷、內文改動還在；存檔後檔案內容相符。
3. 重新匯入 Y；開 Y 專屬條目改字不存 → 撤銷 → 草稿關掉、不另問；世界書沒有該條復活。
4. 重新匯入 Y；開 Y 的角色編輯器改名不存 → 撤銷 → 撤銷確認後編輯器關掉、不另問。
5. 重新匯入 Y；開 X 的角色編輯器改名不存 → 撤銷 → X 編輯器仍開、改名還在。
6. 新桌，世界設定裡開 `TestCards/` 的重構卡 JSON 套用（零 AI）→ 開一條被改寫條目的草稿不改 → 陣容欄撤銷 → 草稿跟上整條還原；被改寫條目整條回原文；該卡若指定了玩家卡，側欄玩家卡清掉；重構結果框（若仍開著）可關閉。
每步截圖存 scratch。同欄衝突以外的競態與跨桌晚到在單元測試驗，不進通道〔模型判斷·未裁決〕。

## 已知限制（罕見不嚴防〔作者裁決 2026-10-11 原則〕）
- 撤銷時世界書讀不了（`entry_changes` 為 `None`）：開著的既有條目草稿直接關，不判斷撤銷有沒有動到它。
- 待定基期間重讀一直失敗：既有條目草稿留在畫面但寫不回、離開被擋，直到重讀成功。
- `removed_character_ids` 沿用既有 `gone_characters`（卡檔查詢失敗時不算已刪），不另補權限類測試。
