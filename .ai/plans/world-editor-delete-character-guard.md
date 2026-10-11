# world-editor-delete-character-guard 方案

## 定案規則〔作者裁決 2026-10-11〕
- 刪掉的不是正在編輯的東西就不關編輯器；刪的正是編輯中那張卡才關。WorldEditor、CardEditor 都照這條。
- 刪的正是編輯中那張卡：不另問草稿，刪除確認就是最後一道。
- CardEditor 隱藏／還原切換：切換前先走既有 `confirmLeave`，選留下就不切換。

## 做法

### 1. 寫回防線放後端指令層（擇 (a)）〔模型判斷·未裁決〕
- 前端存條目的指令 `upsert_worldbook_entry`（`commands/world.rs`）改呼叫 `receipts::upsert_entry_dropping_gone`：在這桌短提交鎖內，可見名單先去重，對每個 id 查 `character_card_gone`——`Ok(true)` 才拿掉；`Ok(false)`（含玩家卡、解析不了但檔在的卡）與查詢錯誤（含 id 不合法）一律保留並記 log，絕不讓整次存檔失敗。有命中才動，濾空改 GM，重用 `scrub_entry`。更新既有條目與新增條目兩條分支都套。
- 比刪除後清理寬：舊資料裡格式合法、本來就沒卡的殘留 id 也會在下次存檔時清掉，這是刻意的〔模型判斷·未裁決〕。
- 不放進 `data::upsert_worldbook_entry`：撤銷匯入插回（`fresh` 比對）、重構套用、匯入合併（`merge_entry_into`）、raw 還原都走資料層；剪了可見度會讓撤銷的 `same()` 比對不成立而多插一份條目。它們的 id 來源在清理成功時已由刪除清理轉換過；清理失敗的殘留是已接受的既有狀況，殘留 id 不擴大誰看得到〔模型判斷·未裁決〕。
- 理由：一道防線涵蓋後端刪完到前端通知之間的空窗、通知前已送出的寫入、清理失敗後關掉再開 WorldEditor；成本是每次存條目對名單每個 id 一次 `try_exists`。
- 鎖序：`delete_character_and_clean` 整段（刪卡檔＋清理）改持同桌 `with_commit`（可重入），與存條目的「檢查＋提交」互斥排序。存條目先拿鎖：檢查時卡還在、照寫，刪除整段排在後面、清理把它拿掉；刪除先拿鎖：存條目輪到時卡檔已不在、必剪，即使清理失敗。刪除前就已寫在書裡的舊引用遇清理失敗仍可能殘留（同既有狀況）。轉條目本來就持整桌獨占，存條目進不來。
- `update_entry_fields` 不寫 `source_cards`，不必另剪；清理失敗留下的來源卡不擴大誰看得到，但仍可能影響匯入合併語意（既有狀況）。
- 既有測試清點：用合法格式 id 當可見名單卻沒寫卡檔的測試都直接呼叫資料層，不經這道防線，不必補卡檔。

### 2. 「確認已刪」由後端精確回答
- 新增查詢指令 `character_card_gone(world_id, character_id) -> bool`，直接包 `data::character_card_gone`；查詢錯誤回 Err＝不確認。
- `characters.remove` 改回三態：`cancelled`／`deleted`／`failed`。`delete_character` 回錯時問 `character_card_gone`：`true` → 當已刪（關卡＋通知），畫面錯誤改顯示「角色已刪除，但部分檔案清理失敗：{原因}」（新 i18n 鍵 `deleteCharacterPartial`，十語系；不沿用 `deleteCharacterCleanupFailed`，那句專講世界書關聯，圖檔殘留時說錯對象）〔模型判斷·未裁決〕；`false` 或查詢失敗 → 不關、不發，保留原錯誤。
- 轉條目（CardEditor `convertCardToWorldbookEntry`）回錯時同樣問 `character_card_gone`，`true` 才當已轉（呼叫 `onConverted`，訊息同上）。

### 3. 關不關（`useWorkspaceNavigationController.ts`）
- `finishRemoval` 拆「善後」與「通知」，各自獨立 try：`noteRemoved`／`refresh` 失敗只報錯，不阻斷關卡與通知。
- 關卡：`setMainView` 的 updater 保持純函式——當下是四種卡片畫面且 `id` 相同才回 `null`。是否實際關了被刪卡在 updater 外用同一判斷（讀最新 mainView 的 ref）決定，只有那時才清 `leaveGuard.current`。單幕閱讀、玩家卡畫面都照「不是它就不關」〔模型判斷·未裁決〕。
- 隱藏／還原（`onArchived`）只關、不發通知。
- 進桌世代：每換一次桌（含 w1→w2→w1）遞增；刪除完成後、善後前、善後每個 await 後都比對，世代變了就不再動發言對象、畫面與通知。

### 4. 刪除通知 `castRemoval`
- 形狀 `{ worldId, generation, ids }`；`worldId` 變了就重設 `ids`；`ids` 累積到 WorldEditor 取走為止，取走／清除要比對 `worldId` 與 `generation`，舊的不清新的。
- 經 `AppWorkspace` → `MainView` 傳給 WorldEditor。WorldEditor 掛載時用 ref 記下當下 `generation` 並跳過，只處理之後的世代；`worldId !== world` 不套用；effect 放在 `if (text === null) return` 之前，守 hooks 順序。

### 5. WorldEditor 收到通知
`useWorldbookEditor.applyCharactersRemoved(ids)`：
- 重讀 `read_worldbook`、`list_characters`、`mechanism_ledger`，三者各自 try/catch、各自世代保護（晚到的舊回應丟掉，成功與失敗都丟，含 cast 重讀）。`entries` 一律直接採後端結果，不在前端修剪（後端已清；清理失敗時殘留的舊 id 不擴大誰看得到，寫回時有 1 的防線）。
- 開著的條目草稿：在同一次提交裡用 `ids`（確認已刪的集合，不比對名單）修剪 `draft` 與 `draftOrigin`：不論目前選哪種可見度，名單都濾掉已刪 id；命中才動，只有「指定角色」濾空才改 gm，本來空名單不改；`setDraft`／`setDraftOrigin` 一律函式式，重讀途中玩家改的字保留。修剪不製造新的髒狀態、保留其餘修改；唯一差異：草稿只在已刪角色上有改動時，修剪後變乾淨，這是合理結果。
- 修剪草稿只為畫面一致（勾選名單已沒有該角色），正確性靠 1。
- world.md 的 `text`／`savedText` 不碰。

### 6. CardEditor
- `newCardColor`：每次 render 寫進 ref，載入 effect 只依賴 `(world, characterId, isNew)`、在 effect 內讀 ref——開新卡或換卡當下取色，刪別張不重跑。
- `toggleArchived` 先 `await confirmLeave()`，選留下就不呼叫 `set_character_archived`。

## 掛點
- 後端：`src-tauri/src/receipts/character_delete.rs`（`upsert_entry_dropping_gone`、刪除整段持鎖）、`src-tauri/src/commands/world.rs`（存條目改走防線）、`src-tauri/src/commands/character.rs`（`character_card_gone` 指令並註冊）。
- `src/features/characters/useCharacterController.ts`：`remove` 三態＋失敗重查。
- `src/controllers/useWorkspaceNavigationController.ts`：`finishRemoval` 拆兩段、純 updater、`castRemoval`。
- `src/views/AppWorkspace.tsx`、`src/views/MainView.tsx`：傳 `castRemoval`。
- `src/features/worldbook/WorldEditor.tsx`、`useWorldbookEditor.ts`、`worldbook-model.ts`（草稿修剪純函式）。
- `src/features/characters/CardEditor.tsx`：顏色 ref、`toggleArchived` 先問、轉條目失敗重查。
- `src/i18n/*`：`deleteCharacterPartial`。

## 測試
- Rust（`receipts/upsert_guard_tests.rs`）：交錯測試兩支（存條目檢查後暫停、同時刪卡且清理成功；刪除持鎖中清理失敗、存條目等在後面）；upsert 名單含已刪 id → 拿掉；只剩已刪 id → gm；含玩家卡 id（卡檔在、不在 `list_characters`）與解析不了但檔在的卡 id → 保留；`try_exists` 錯誤 → 保留（權限測試，root 下略過）；沒命中原樣。`character_card_gone` 指令三態。
- `worldbook-model.test.ts`：草稿修剪——命中才動、濾空改 gm、沒命中原樣、本來空名單不改、非 characters 不動。
- `WorldEditor.test.tsx`：
  - 送通知後編輯器在、world.md 文字與新條目草稿都在，畫面上 `entries`、勾選名單換成重讀結果。
  - 未改過的既有條目草稿（指定含 B），刪 B 後離開 → 不呼叫 upsert，`draftDirty` 仍 false。
  - 草稿名單含玩家卡 id／解析不了的卡 id，刪別張後重讀、帳本開關、自動存 → payload 仍保留它們（前端不剪非刪除 id）。
  - 只改過 B 勾選的草稿，刪 B 後變乾淨；同時改過內文的仍髒、內文保留。
  - 重讀途中改字不被蓋掉；晚到的舊重讀回應被丟掉。
  - 重讀被拒不炸、草稿仍照 `ids` 修剪。
  - 連刪兩張（兩個世代或一次累積）都處理；別桌通知不套用；已有 `castRemoval` 時掛載 → 不重讀不修剪。
- `src/features/characters/removal-navigation.test.ts`（controllers/ 不放測試，跟著角色刪除住）：`world` 刪任一角色不關、通知遞增；`character A` 刪 B 不關、刪 A 關且清 guard；`new-character`、`scene`、`player` 刪角色不變；確認框取消不動；`remove` 失敗且 `character_card_gone` 為 false → 不關不發、保留原錯；失敗但為 true → 關並發、顯示部分清理失敗；查詢失敗 → 不發；隱藏／還原只關不發；確認期間換了畫面 → 依當下畫面判斷；`noteRemoved` 失敗仍關並發；換桌後 `ids` 重設。
- `CardEditor.test.tsx`：rerender 換 `newCardColor` 後既有卡與新卡草稿都在、不重呼叫 `read_character`；換卡／開新卡時取到當下顏色；未存時按隱藏先問，選留下不呼叫 `set_character_archived`；轉條目回錯但卡已不在 → 呼叫 `onConverted`。
- `npm run verify` 全綠。

## 驗收：測試通道實測
`npm run harness:build` → `node scripts/harness.mjs launch --fresh --root <scratch>`，零 AI：
1. 建角色卡 A、B、C、D、E，隱藏 B、C、D、E；世界書新增一條可見度「指定角色＝B」並存檔。
2. 開世界設定：改 world.md 不存、開一條新條目草稿；從側欄隱藏區刪 B（對話窗指令按確認）→ 世界設定仍開著、文字與草稿都在；那條條目變 GM；勾選名單沒有 B。
3. 開 A 的編輯器改名不存，刪 C → 編輯器仍開、改名還在；開新卡打字不存，刪 D → 新卡表單還在、顏色不變。
4. 開 E 的編輯器改名不存，從側欄刪 E → 刪除確認後編輯器關掉、不另問。
5. A 有未存改動時按隱藏 → 跳離開詢問，選留下不隱藏、改動還在。
每步截圖存 scratch。

## 範圍外
- 撤銷上次匯入的兩個問題（重掛載 WorldEditor 丟草稿、CardEditor 指著被撤銷的卡）由主線另立案。
- 刪檔失敗的錯誤由角色 controller 直接報，跨桌後晚到時可能顯示在新桌的錯誤列；只多一則提示、不動資料與畫面〔模型判斷·未裁決〕。
