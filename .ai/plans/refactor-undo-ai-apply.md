# refactor-undo-ai-apply 方案

> 狀態：A 定案〔作者裁決 2026-10-29〕；第 1–3 輪審查（Sol、opus-review）意見已併入，三方共識；2026-10-29 結案進 main（驗收兩輪 Sol、opus-review PASS）。

## 查因結論

**刻意設計，但設計沒做完整。**

- 刻意：`src/features/refactor/useRefactorWorkflow.ts:820` `recordReceipt: origin !== "ai"`，來自 879e6145（refactor-output-redesign）。依據是該案驗收標準第 7 條〔作者裁決 2026-08-11〕：「AI 重構套用無還原……還原只留『匯入重構卡』路徑（花額度的成果不能被誤按 undo 抹掉）」，寫進 `.ai/reference/CARD-REFACTOR-SPEC.md:11`、`:51`、`:125`。
- 沒做完整：只做了「不記收據」，「復原上次匯入」照樣撤收據清單最後一筆（`src/App.tsx:763-781` → `src-tauri/src/receipts/undo.rs` `undo_last`），撤到重構**之前**的匯入。那筆收據的 `added_state_keys` 還在樹上，狀態樹被清掉；重構寫的機制表、`refactor_mode`、`statusbar_retired`、角色卡不在那筆收據裡，全部留著——半重構半舊。

## 業界做法與差距

undo 堆疊一致的做法只有兩種：每個變動都進堆疊，或做了不能撤的操作時截斷堆疊、之後的 undo 不跨過它（Excel 跑巨集清 undo）。我們現在是「不記這筆但堆疊照用」，會撤錯對象。本案照第 7 條走第二種：AI 套用在堆疊上放一道屏障。

## 定案：AI 套用設屏障，復原到此停〔作者裁決 2026-10-29〕

玩家看到的：AI 重構套用後，陣容欄選單沒有「復原上次匯入」；之後再匯入的卡照樣出現復原、可逐筆撤，撤到重構那一步選單項目就消失。想回原卡＝按「重構」重跑（既有的清回原卡流程）或開新桌重匯。「匯入重構卡」套用照舊可復原。

### 1. 屏障收據的資料形狀
- 沿用 `ImportReceipt`，`kind: "refactor_ai"`，`label` 用既有 `UiMsg::ReceiptRefactorApply`，`timestamp` 照填；其餘欄位全空（都有 `serde(default)`，不加新欄位）。
- 常數 `REFACTOR_AI_BARRIER_KIND` 放 `receipts.rs`。kind 已在 `ImportReceiptSummary` 裡回前端，後端契約不必改；前端 `src/features/import/useImportController.ts:81` 的 `kind` 型別現在只寫 `"character" | "worldbook"`，補成 `"character" | "worldbook" | "refactor" | "refactor_ai"`，同檔定前端常數。
- 新函式（`receipts/record.rs`）：`push_refactor_barrier(root, world_id, label, held) -> DataResult<()>`，用 `read_receipts_checked`＋原子寫；`pop_refactor_barrier(root, world_id, held) -> DataResult<()>`，最後一筆是屏障才彈，否則不動。

### 2. 套用時機：先放屏障再套用（`refactor/apply_record.rs`）
- 前端參數改成必填 `origin: "ai" | "card"`（`RefactorApplyOrigin` enum，serde snake_case），取代 `record_receipt: Option<bool>`：沒有預設值，呼叫端不會默默拿到錯的收據模式。`commands/refactor.rs`、`refactor/reset.rs apply_to_table`、`apply_and_record` 一路換型別；`useRefactorWorkflow.ts:820` 改傳 `origin: origin === "ai" ? "ai" : "card"`。
- `origin == Ai`：在 `snapshot_refactor` 之後、`apply_with_assets` 之前 `push_refactor_barrier`。寫不進去（含收據檔讀壞）就回錯、什麼都不套用——零寫入〔模型判斷·未裁決〕。收據檔壞掉的桌從此不能 AI 重構，屬罕見狀況，照輕防處理。
- 成功：不記完整收據，屏障留著。
- 中途失敗（既有流程照走：progress 記收據 → `rollback_last_import`）：
  - 寫入前就被拒（`RefactorRecord::Nothing`，例 `refactor/apply.rs:95` `PlayerCardExists`）→ `pop_refactor_barrier`，回原錯。桌子沒變，不能留屏障擋住更早的匯入。
  - 回滾成功 → `pop_refactor_barrier`，收據清單回到套用前原樣，回原錯。
  - 上兩項彈不掉只記 log（屏障多留一道只是擋住更早的復原，不壞桌）。
  - 回滾失敗（`RefactorApplyPartial`）→ progress 收據在屏障上面，玩家可手動撤它，撤完停在屏障。
  - progress 收據沒寫成（`RefactorApplyPartialNoReceipt`）→ 屏障在最上面，復原鈕不出現，正好擋住撤到別次匯入。
- `origin == Card`：維持現行完整收據，不放屏障。

### 3. 判斷「能不能撤」的位置
- 後端（權威）：`receipts/undo.rs undo_last` 在 `pop` 之後、比對 `expected` 之前，若 `receipt.kind` 是屏障就回 `UiMsg::UndoBlockedByRefactor`（新變體），磁碟不寫；`strict`（自動回滾）與玩家撤銷都適用。十語系譯文（位置見下）文意：「AI 重構套用後，之前的匯入無法再復原；要回到原卡請按『重構』重跑，或開新桌。」只在前端判斷落後（換桌競態）時才看得到。
- 前端：`useImportController.ts` 匯出共用函式 `canUndoLastReceipt(receipts)`（非空且最後一筆不是屏障），`src/views/AppWorkspace.tsx:264` `canUndoImport` 與 `App.tsx:763 runUndoLastImport` 開頭都用它，後者是屏障就直接 return（不跳確認框）。
- 錯誤碼登記在 `src/i18n/features/backend-msg.ts`（`import_receipt_changed` 那一區，約 :235），十語系譯文 `be_undo_blocked_by_refactor` 寫在 `src/i18n/features/backend-msg-table.ts`，兩檔都改。
- 其他會掛到「最後一筆」的寫入要跳過屏障，不然資料會掛在撤不掉的那筆上：`record_posted_opening`（`import == None` 走 `receipts.last_mut()` 那支）與 `record_last_import_rename`，最後一筆是屏障時當作沒有收據可掛（照各自既有的「沒收據」分支回傳）。

### 4. 屏障之後的匯入
屏障只是清單裡的一筆：之後的角色卡／世界書匯入照常 append，`undo_last` 照常逐筆彈，彈到屏障被拒。`import_replays` 只認 `character | worldbook`（`receipts/sources.rs:99`），屏障不影響「重新重構」要重匯的原檔清單。
- 第二張卡的匯入路由：`useImportController.ts:445-448` 把收據 kind 交給 `decideImportRoute`（`import-routing.ts:25`）。沒有匯入收據的桌 AI 重構後只剩屏障，再匯卡會從 direct 變 ask；傳進去前只扣掉屏障 `refactor_ai`；匯入重構卡留下的 `refactor` 收據是真匯入，照 `import-routing.ts:12-13`「這桌一旦匯過東西，一律開框」（2026-08-06 立規）照算，行為不變。`ImportReceiptKind` 型別補 `"refactor"`（執行期本來就會傳進來）。`needsFallback` 的「收據為空」也改看扣掉屏障後的清單。

### 4b. 套用失敗後的前端刷新（`useRefactorWorkflow.ts:835` catch）
現在套用失敗只顯示錯誤、不重讀；留下 progress 收據（`RefactorApplyPartial`）或屏障（`RefactorApplyPartialNoReceipt`、屏障彈不掉）時，陣容欄選單還是舊清單：該出現的手動復原沒出現，或顯示一個會被拒的復原。改成：後端關卡之外的失敗（不是 `REFACTOR_PLAYED`、`REFACTOR_EMPTY_SELECTION`、`refactor_assets_gone`），在 `live()` 為真時依序做下列各步，每步各自捕錯、不互相擋：
1. `flushed = await flushDraftForRecovery()`（拋錯當 false）。照 `resetForRerun`（`useRefactorWorkflow.ts:248`）先存開著的既有條目草稿：下一步傳 `null` 會讓 `applyWorldChange`（`useWorldbookEditor.ts:171-174`）把它直接關掉不存。但不能直接用現成的 `flushDraft`（＝`flushAndCloseExistingDraft`）：`persistDraft` 回 `"gone"`（`useWorldbookEditor.ts:276-282`，條目已被部分套用刪掉）時會自己關草稿，flush 仍回 true，未存文字就沒了。所以 `useWorldbookEditor` 新增 `flushExistingDraftForRecovery()`，經 `WorldEditor` 傳成 hook 的新依賴 `flushDraftForRecovery`：
   - 草稿乾淨或沒開既有條目 → 照 `flushAndCloseExistingDraft` 關掉、回 true。
   - 有改動 → `persistDraft(latest, { closeOnGone: false })`（新選項，預設 true，既有呼叫端行為不變）。`"saved"` → 關草稿、回 true；`"gone"` 或 `"failed"` → 草稿留著不關（訊息照 persistDraft 既有的顯示）、回 false。
   - 部分套用改寫過的條目存草稿時以玩家草稿為準。實作時核對 `"gone"` 後的清單重讀不會連帶把草稿關掉。
2. `flushed` 且 `live()` → `try { await refreshAfterApply(live, null) }`（走 `App.tsx refreshAfterRefactorApplied`）；拋錯只記 log。`flushed` 為 false 就跳過，草稿照留不關。
3. `live()` → `try { await refreshReceipts() }`：不管前兩步成敗都試一次（第 2 步若已重讀就是重複一次，無妨）。`refreshReceipts` 是新注入的依賴，跟 `refreshAfterApply` 同樣由 `App.tsx` 經 `WorldEditor` 傳進 hook（App 端是 `imports.refreshReceipts(table)`）。
4. 最後 `live()` 才設狀態列為原錯誤字串，不被刷新的錯誤蓋掉。
- 已知限制：草稿存不起來時桌面資料不刷新（只刷收據），部分套用的畫面要等重開世界設定才對上〔模型判斷·未裁決〕，屬罕見情形。

### 5. 重構覆蓋重跑
重構過、未遊玩的桌再按重構，既有 `reset_to_import_source` 會用匯入原檔在臨時根重建整桌、再整桌交換：新桌的收據是重匯產生的，舊屏障隨舊桌消失，不必特別處理。重跑套用時再放一道新屏障。已遊玩的桌重構被擋，屏障不變。

### 6. 匯入重構卡路徑不受影響
`origin == Card` 走完整收據，可復原。重構過的桌匯入重構卡也會先清回原卡（同上，屏障隨之消失），所以不會出現「重構卡收據疊在屏障上」需要特別處理的情形；就算疊上去（例如 AI 套用存檔失敗、桌子沒被認成已重構），也只是先撤重構卡、再停在屏障。

### 7. 不處理
- 這版之前就 AI 重構過的桌沒有屏障，復原照舊會撤錯；發佈前不相容舊桌，不補遷移。

## 已知限制
- 補救後留著的草稿對應條目已被刪：按存檔會被拒，但表單照留、顯示「條目已不在」，玩家可複製文字〔作者裁決 2026-10-29〕；切換到別的條目或離開世界設定仍會關掉這份草稿、文字不保留〔作者裁決 2026-10-29〕。
- 回滾失敗後玩家手動撤掉 progress 收據，屏障仍在，更早的匯入撤不了〔模型判斷·未裁決〕：桌子已回到套用前，只是少了往回撤的能力；要回原卡照樣能重構重跑或開新桌。

## 驗收

### Rust 單元測試（`refactor/apply_record/tests.rs`、`receipts/tests`）
收據比對一律用「解析後結構等值且順序不變」（缺檔與空清單視為相同），不比 bytes——現行 pop 是讀成結構再 pretty-print。
1. 回歸（在 main 上會紅）：世界書匯入 → AI 套用 → `undo_expected_import` 回 `UndoBlockedByRefactor`；`state.json`、世界書、角色目錄、介面殼逐檔與撤銷前相同，收據結構不變。
2. AI 套用 → 匯入一張角色卡 → 復原撤掉那張角色卡 → 再復原被拒。
2b. AI 套用 → 匯入一份世界書 → 復原：狀態樹、機制、世界書都等於那次匯入前（AI 套用後）的樣子，屏障仍在最後一筆 → 再復原被拒。
3. 中途失敗：
   - 寫入前被拒（`PlayerCardExists`）：先有一筆世界書匯入；在測試裡用資料層直接建一張角色並把 `player_card_id` 指向它（不要先跑一次 Card 套用產生玩家卡，那樣最後一筆會變 `refactor` 收據、驗不到撤世界書匯入），再用指定玩家卡的產物走 AI 套用。套用回錯後收據結構不變、復原照樣撤得掉那筆世界書匯入。
   - 回滾成功：收據與套用前結構相同（沒有殘留屏障）。
   - 回滾失敗：最後一筆是 progress 收據、下面是屏障。
   - progress 收據沒寫成：最後一筆是屏障。用 `data::WriteFailGuard::partial_ending_after("import-receipts.json.tmp", 1, 1)`（`data/world_file.rs:182`）讓屏障那次寫入過、progress 那次失敗；不用 `apply_record/tests.rs:200` 的目錄佔位招（會讓屏障先寫不進去）。
4. 屏障寫不進去（收據檔放壞 JSON）：套用回錯、桌子零寫入。
5. Card 路徑：照舊完整收據、可撤（既有測試照過，改傳 `origin`）。
6. `import_replays` 在有屏障時回的清單與沒有屏障時相同；AI 套用後 `reset_to_import_source` 的新桌收據不含屏障。
7. 屏障在最上面時 `record_posted_opening(import=None)`、`record_last_import_rename` 不改動屏障。
8. `rollback_last_import` 遇到屏障在最上面也拒撤。

### 前端測試（`src/features/card-interface/refactor-undo-flow.test.tsx` 等）
- 最後一筆是屏障：陣容欄選單沒有復原項目；屏障上面有角色卡收據：有復原項目。
- `refactor_apply` 的參數：AI 結果卡帶 `origin: "ai"`、重構卡帶 `origin: "card"`，不再帶 `recordReceipt`。
- 後端回 `undo_blocked_by_refactor`：狀態列顯示翻譯後的訊息並重讀收據清單。
- 套用回 `RefactorApplyPartial`／`RefactorApplyPartialNoReceipt`：會重讀收據與桌面資料（`list_import_receipts` 被叫到），狀態列保留原錯誤。
- 套用失敗、`refreshAfterApply` reject（例 `read_state` 失敗）：`list_import_receipts` 仍被叫到，狀態列是原錯誤。
- 既有條目草稿有改動時套用失敗：改動有存下來（upsert 帶草稿內容，發生在刷新之前）；存草稿失敗時不刷新桌面、草稿仍開著、收據仍重讀、狀態列是原錯誤。
- 部分套用刪掉了草稿對應的條目（upsert 回 `worldbook_entry_gone`）：草稿仍開著、未存文字還在，不刷新桌面，收據仍重讀，狀態列是原錯誤。
- `persistDraft` 預設行為不變：既有的切換條目遇到 `"gone"` 照舊關草稿（既有測試照過）。
- `canUndoLastReceipt` 單元測試；`decideImportRoute` 的呼叫端：只有屏障的桌走 direct，有 `refactor` 收據的桌照舊 ask。

### 測試通道實測（`npm run harness:build` → `node scripts/harness.mjs`，獨立 `--root`）
1. 開新桌，匯入 `TestCards/` 一張帶世界書的卡，截圖陣容欄選單（有復原）。
2. 低階模型跑 AI 重構 → 全部套用 → 截圖陣容欄選單（沒有復原）、狀態欄、世界書。
3. 匯入另一張角色卡 → 選單出現復原 → 復原 → 角色卡消失、選單項目又消失；狀態欄與世界書和第 2 步截圖相同。
4. 同一張原卡開另一桌，匯出第 2 步的重構卡、在新桌匯入重構卡套用 → 選單有復原 → 復原 → 回到只有原卡匯入的狀態。
5. 第 2 步那桌再按重構（重跑）→ 清回原卡後選單恢復有復原（原卡匯入收據），取消重構不套用即可結束。

### 其他
- `npm run verify` 全綠；規格 `CARD-REFACTOR-SPEC.md:11` 補一句「AI 套用在收據清單放屏障，復原不跨過它」，`:51`、`:125` 不動。

## 預估改動
後端約 90 行（屏障兩函式、undo 判斷、origin 換型別、Nothing／回滾後彈屏障、兩處跳過屏障、UiMsg 一個變體）、前端約 50 行（共用判斷、kind 型別與路由過濾、失敗後存草稿與分步刷新、注入 refreshReceipts）、i18n 十語系各 1 行、測試約 250 行。

## 施工與方案的出入
- 屏障寫不進去回新變體 `UiMsg::RefactorBarrierFailed`（方案只寫「回錯」），十語系同兩檔。
- 「這次的收據沒寫成」測試改用 `RenameFailGuard::fail_ending_after("import-receipts.json", 1, 1)`：整檔寫入的注入一次只能打一種結尾，`.md` 已拿去讓套用中途失敗。
- 補救後留著的草稿若對應條目已不在清單，原本畫不出來（表單只畫在條目那一列）；`WorldbookSection` 改成把這種草稿畫在清單底部（同新條目的位置）。
- `refreshReceipts` 由 `AppWorkspace` 組（它手上就有 `imports` 與 `table`），經 `MainView` → `WorldEditor` 傳進 hook，`App.tsx` 不經手。
- 路由過濾抽成 `useImportController` 匯出的 `importRouteKinds`，方便單測。
- 玩家親手按存檔遇到條目已不在時不關草稿（`saveEntry` 一律 `closeOnGone: false`，不看前端清單）〔作者裁決 2026-10-29〕；切換、離開照舊關。
- `App.tsx` 復原失敗時，`undo_blocked_by_refactor` 與 `import_receipt_changed` 一樣重讀收據。

## 驗收結果（2026-10-29）
- `npm run verify` 13 步綠；cargo test 2458 過；屏障測試 10 項（`refactor/apply_record/barrier_tests.rs`），拿掉 undo 屏障判斷時其中 5 項紅。
- 測試通道（北境驿站 PNG 匯入成世界書、claude-haiku-5-5 跑 AI 重構）五步全過：匯入後有復原；AI 套用後收據為 worldbook＋refactor_ai、選單沒有復原；再匯一張角色卡出現復原、撤掉後選單又沒有，狀態、機制、玩法標記、退場標記、世界書與套用後完全相同，後端直接呼叫撤銷回 `undo_blocked_by_refactor`；另一桌匯入重構卡套用後有復原、撤銷後狀態相同、世界書 23 條逐 uid 內容相同；原桌重構重跑清回後收據只剩 worldbook、選單恢復有復原。
- 觀察（本案範圍外、既有行為）：匯入重構卡撤銷後，世界書條目內容逐 uid 相同但排列順序反過來。
