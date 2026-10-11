# character-delete-visibility-cleanup 施工方案

交接見 [handoffs/archive/character-delete-visibility-cleanup.md](../handoffs/archive/character-delete-visibility-cleanup.md)。行號以分支起點 `0378e93` 為準，Rust 路徑相對 `src-tauri/src/`。

## 一、現況

### 世界書裡存角色 id 的地方
條目原始 JSON 的 `extensions.table_tavern` 底下只有兩欄：
- `visibility`：`{"characters": [id…]}`（讀取 `data/worldbook.rs:105-127`，陣列全是字串才算 `Characters`，否則退回 `Gm`）。
- `source_cards`：`[id…]`（`data/worldbook/book_import.rs:60`；讀寫 `raw_entries.rs:197-210`，空清單移除欄位）。用途：跟卡匯出的歸屬（`raw_entries.rs:178-195`）、`Gm` 條目的「落定」（`book_import.rs:342-358`）。

收據裡也存著這兩欄的值：`worldbook_entries` 的指紋（`receipts.rs:461-479`，含可見度）、`visibility_restores` 的前後值、`rewritten_entries`／`deleted_entries`／`deleted_entries_raw` 的條目快照。WI 計時檔的視角鍵 `char:<id>`（`data/world_info_store/mod.rs:37`）也有角色 id。

### 刪角色的掛點（只有桌面版）
| 入口 | 位置 | 鎖 |
|---|---|---|
| 玩家刪角色（含隱藏區、玩家卡） | `commands/character.rs:68-77` → `data::delete_character`（`data/character.rs:405-421`，先刪 md 再刪圖庫與圖）；前端 `useCharacterController.ts:289`、`:310` | 共用許可 |
| 角色卡轉世界書條目 | `commands/world.rs:120-131` → `data/worldbook.rs:729-776`（新增條目後刪卡） | 資料層自取獨占 |
| 撤銷匯入／重構自動回滾 | `receipts.rs:807-838`（`character_id`、`character_ids`） | 呼叫端持獨占 |

封存（`set_character_archived`）不是刪除，不處理〔模型判斷·未裁決〕。

網頁版 `web/` 沒有刪角色：一份存檔只玩一張卡，存檔整份刪（`web/src/features/saves/save-store.ts:140`），也沒有桌世界書的可見度名單（`web/src` 搜不到 `visibility`／`table_tavern`）。網頁版不改〔模型判斷·未裁決〕。

### 現在刪掉角色會怎樣
名單裡的 id 留著。只剩這個 id 的條目讀成 `Characters([不存在的 id])`：角色都看不到，GM 掃描當機密條目（`limited`／`confidential()`），遞迴不進公開緩衝、outlet 被標私密。編輯器打開該條是「指定角色」但沒有任何勾選框是勾的。id 由 `data::new_id()` 產生，不會被新角色撿到。

## 二、做法

### 1. 判準：卡檔已不在才清
三條路都一樣：刪除動作結束後，`characters/<id>.md` 已不在就清該 id，不看 `delete_character` 回 `Ok` 還是 `Err`。存在與否用可回錯的檢查（`Path::try_exists`）：查不到（權限、I/O 錯誤）就記 log、不清，不把讀取錯誤當成已刪；刪卡本身成功時回 `worldbook_cleanup_failed = true`，刪卡本身失敗照舊回原錯。撤銷流程照撤銷的嚴格／非嚴格規則處理。`delete_character` 先刪 md 再刪圖庫與圖，後段失敗時卡其實已經不在桌上；md 還在（刪 md 本身失敗）就不清，免得還在的角色失去可見度。

### 2. 清理規則（世界書路與角色卡路一視同仁）
一個純函式套在單條值上，世界書與收據快照共用：
- `visibility`：只動讀得成 `Characters` 的（物件形、陣列全字串）。拿掉 `ids` 裡的 id（重複出現的一併拿掉）；剩下的照原順序留著；剩空就寫成字串 `"gm"`〔作者裁決 2026-10-11〕。`"gm"`／`"public"`、讀不懂的格式、本來就空的名單（不含要刪的 id）都不碰〔模型判斷·未裁決〕。
- `source_cards`：拿掉 `ids` 裡的 id，空了移除欄位（沿用 `set_source_cards` 的語意）。不管可見度是什麼都清。
- 條目不論從哪條路來（角色卡路、世界書路、編輯器手建、重構）只看欄位內容。

世界書：新檔 `data/worldbook/character_scrub.rs`，`scrub_character_ids(root, world_id, ids) -> DataResult<Vec<ScrubbedEntry>>`（`ScrubbedEntry { uid, before, after }`，before／after 是條目原始 JSON，供收據轉換用）。在 `with_commit` 內讀、改、有變動才寫；整檔原子寫（`commit_world_write_atomic`：同目錄暫存檔＋fsync＋改名，失敗原檔不動）。uid 不變，不經 `prune_uids`；在 `worldbook.rs` 開一個原子寫的入口，其他世界書寫入路不改〔模型判斷·未裁決〕。同一組 id 再跑一次沒有變動、不寫檔。

### 3. 刪角色時收據一致轉換
目的：撤銷時的比對（指紋、可見度還原、快照插回）都以「清理後的世界」為準，不因刪角色而把條目誤判成玩家改過，也不把已刪的 id 插回來。新檔 `receipts/character_delete.rs`，在清世界書的同一個 `with_commit` 內做；世界書清理回 `Err` 就不做收據轉換，直接回 `worldbook_cleanup_failed = true`。

**快照**：所有收據的 `rewritten_entries`、`deleted_entries`（精簡條目的 `visibility`）、`deleted_entries_raw`（原始 JSON 的 `visibility` 與 `source_cards`）一律套清理規則，不受世界書這次有沒有命中影響（世界書沒有 A、只有快照帶 A 也要清）。

**可抵達值模擬**（給指紋與 `visibility_restores` 用）：對每個 uid，沿撤銷順序由新到舊推「撤到這筆收據時，這個 uid 上是什麼值」`R: Option<值>`（`None`＝uid 上沒有條目）。值一律記刪角色清理前的完整條目（精簡欄位＋原始 `visibility`＋`source_cards`），比對時用清理前的值，換成的結果用清理後的值；收據的 before／after 與快照都套同一個清理規則，清理規則彼此可交換，所以「先模擬再清」等於「撤銷時實際看到的值」。
- 起點：`R` ＝ 世界書清理前的目前條目（不在就是 `None`）。
- 處理收據 Ri 時，照撤銷步驟的順序：
  1. **1a 可見度還原**：`R` 是 `Some(v)` 且 restore 的 after 等於 v 的 `visibility` 與 `source_cards`（比對方式同 `apply_visibility_restore`，`raw_entries.rs:138-142`）→ 這筆 restore 的 before 與 after 兩邊都套清理規則，`R` 換成「v 套上 before」（before 為 `None`＝移除該欄位，讀成 `Gm`，比照 `raw_entries.rs:148-151`）。沒命中就不轉換、`R` 不變（撤銷時照現行規則保留）。不採「restore 一律轉換」：玩家改過的值清理後可能剛好等於轉換後的 after，撤銷會蓋掉玩家的修改〔模型判斷·未裁決〕。
  2. **2 指紋**（Ri 的 `worldbook_entries`）：`R` 是 `Some(v)` 且記錄的指紋等於 v 的指紋 → 換成 v 清理後的指紋，`R` 變 `None`（撤銷會刪掉它）。沒命中＝玩家改過，不動、`R` 不變。
  3. **3 改寫快照**（Ri 的 `rewritten_entries`）：`R` 是 `Some(v)`（撤銷時 uid 還在才覆寫）→ `R` 換成 v 套上快照（精簡欄位取快照，`source_cards` 沿用 v，同 `upsert_worldbook_entry` 的效果）。`None` 不變。
  4. **3a 刪除快照**（Ri 的 `deleted_entries`）：只有 `R` 是 `None` 才設成快照值；`R` 是 `Some` 表示原 uid 被佔：撤銷時要嘛判定快照已存在而略過，要嘛改插新 uid（`raw_entries.rs:98-106`），兩者原 uid 上的值都不變，`R` 不動。快照清理後與桌上某條全同會被當成已插回而略過，只是少一條重複，不遺失資料。快照值的來源照撤銷的判斷（`receipts.rs:898`）：`deleted_entries_raw` 與 `deleted_entries` 長度相同才用原始 JSON，否則用精簡快照（沒有 `source_cards`）。
  5. **撤銷自己的清理**：`R` 是 `Some(v)` → 對 v 套清理規則，id 取 Ri 的 `character_id`／`character_ids`（撤銷這筆時會刪掉並清理這些角色，見二之 4）。
- 各候選只用來比對 `worldbook_entries` 的指紋與 restore 的 after，不直接寫進世界書。
- 收據內容有變才整份原子寫一次；沒變就不寫。

逐一驗證的情境（各加測試）：
- 公開升級：R1 建 E`[A]`；R2 匯入同內容卡 B、來源明寫 public，E 變 `Public`＋來源卡 `[A, B]`；刪 B。`R`={`Public`＋`[A, B]`}，R2 的 restore 命中、轉成 after `Public`＋`[A]`、before `[A]`＋`[A]`；撤銷 R2 → E 回到 `[A]`。
- 鏈式：R1 建 E`[A]`；R2 合併 B，E 變 `[A, B]`；刪 A。R2 的 restore 命中，`R` 變成 {`[A]`}；R1 的指紋對上 `[A]`，換成 `gm` 的指紋。撤銷 R2 → E 回 `gm`；撤銷 R1 → E 被刪，不留孤兒。
- 玩家改過（不可抵達）：同上但刪 A 前玩家手動把 E 改成 GM。`R`={`Gm`＋`[A, B]`}，R2 的 after `[A, B]` 對不上，`R` 不變；R1 的指紋 `[A]` 對不上 `Gm`，不換。撤銷 R2 保留 E、撤銷 R1 也保留 E（照 `receipts.rs:842`、`:857` 的現行規則）。
- 重構刪除：R1 匯入 A 建 E`[A]`；R2 重構刪掉 E（記在 `deleted_entries`）；刪 A。世界書沒有 E，`R` 起點 `None`；R2 的刪除快照轉成 `gm`、`R`＝`[A]`；R1 的指紋對上，換成 `gm` 的。撤銷 R2 插回 `gm` 的 E；撤銷 R1 → E 被刪。
- 重構改寫：R1 建 E`[A]`；R2 重構改寫 E（記在 `rewritten_entries`）；刪 A。R2 的改寫快照轉成 `gm`、`R` 換成 `[A]` 的快照值；R1 的指紋對上，換成 `gm` 的。撤銷 R2 覆寫回 `gm`；撤銷 R1 → E 被刪。
- uid 碰撞：R1 建 E`[A]`；R2 重構刪掉 E；E′ 佔到 E 的原 uid（`next_uid` 是最大 uid＋1，`data/worldbook.rs:344`，測試直接寫原始 JSON 放 E′，GM、精簡欄位與 E 相同、至少一個原始欄位與清理後快照不同）；刪 A。`R` 起點是 E′，R2 的 3a 不動 `R`；R1 的指紋（`[A]`）對不上 E′，不換。撤銷 R2 把 E 插到新 uid、E′ 保留；撤銷 R1 不刪 E′。另一條：E′ 與清理後快照全同，撤銷 R2 判定已插回而略過；撤銷 R1 不刪 E′。
- 撤銷自己的清理：R1 建 E`[A]`；R2 匯入 B；玩家把 B 加進 E 成 `[A, B]`；刪 A。`R`＝`[A, B]`，R2 的第 5 步清掉 B 得 `[A]`；R1 的指紋對上，換成 `gm` 的。實際：刪 A 後 E 是 `[B]`，撤銷 R2 刪 B 並清理 → `gm`；撤銷 R1 → E 被刪。

### 4. 掛點
- **玩家刪角色**（`commands/character.rs:68-77`）：共用許可內呼叫 `data::delete_character`；不論結果，md 確定不在就呼叫清理（世界書＋收據）。`delete_character` 回 `Err` 時清完照樣回原錯；`Ok` 時回 `CharacterDeleteOutcome { worldbook_cleanup_failed: bool }`（snake_case，比照 `book_failed`／`image_dropped`）。兩個玩家同時刪不同卡（都持共用許可）由 `with_commit` 序列化，收據轉換不互蓋。
- **角色卡轉條目**：command 自己取獨占（`try_world_exclusive`，忙碌時回 `WorldBusy` 照舊），持有到轉換、清理、收據轉換都做完。資料層拆出接受既有獨占的入口（例如 `character_to_worldbook_entry_held(&WorldExclusive, …)`），原本自取獨占的寫法拿掉；既有直接呼叫 `data::character_to_worldbook_entry` 的測試（`data/worldbook/tests.rs:617-791` 一帶、`import/export/tests.rs:345`）改走 `_held` 入口。`worldbook.rs:774` 刪卡回 `Err` 時，md 已不在照樣清理，清完回原錯；成功回同一個結構。
- **撤銷**（`receipts.rs:807-838`）：收集步驟 1 處理過、md 已不在的 id（非嚴格模式 `kept` 回 `None` 但 md 已刪的也算）。步驟 3a 之後、步驟 4 之前呼叫 `scrub_character_ids`，包在 `kept(strict, …)` 裡。撤銷不做收據轉換：這次刪的 id 都是本收據新建的角色，較早的收據不可能記著它們（前後值與快照都早於這次匯入；本收據自己合併擴大的條目已由步驟 1a 還原），本收據的快照也是套用前的值，不含這些 id，換指紋的條件永遠不成立。

### 5. 收據檔一律原子寫
`write_receipts`（`receipts.rs:256-260`，`:277`、`:335`、`:725` 在用）改用 `commit_world_write_atomic`，與 `pop_receipts` 一致；寫到一半或改名失敗時整份舊收據原樣留著。收據追加不必包進 `with_commit`：寫收據的路徑都持 `WorldExclusive`，與刪角色的共用許可互斥。

### 6. 前端
- 新增一條 i18n 字串（十語系），意思是「角色已刪除，但世界書的角色關聯未清理完成，可到世界書檢查」。
- `useCharacterController.ts` 的 `remove`／`removePlayer`：讀 `result?.worldbook_cleanup_failed`（既有測試 handler 回 `undefined`），為真時照常當成刪除成功、後續善後照做，另用 `onError` 顯示提示。`removePlayer` 善後（`set_player_card` 等）之後提示仍要看得到。
- `CardEditor.tsx:331` 轉條目：`CardEditor` 沒有 `onError`，`setMessage` 會隨卸載消失，提示接在 `convertCardDone` 的 `showMessage` 文字後面。

## 三、資料安全
- 順序一律「先刪卡、再清理」。md 還在就不清，不會讓還在的角色失去可見度；清理失敗＝回到現在的行為（名單留著已刪的 id）。
- 世界書與收據各自整檔原子寫，但兩檔之間沒有跨檔一致：
  - 世界書清理失敗 → 不碰收據，回 `worldbook_cleanup_failed = true`；兩檔都是刪角色前的樣子，跟今天一樣。
  - 世界書寫成、收據寫失敗 → 也回 `worldbook_cleanup_failed = true`。收據仍是轉換前的內容：撤銷時清過的條目被當成玩家改過而保留，可見度還原對不上而保留，而且沒轉換的重構快照撤銷時會把已刪的 id 插回來。插回的名單只剩已刪的 id 時只有 GM 看得到（跟今天一樣）；名單裡另有還在的角色，那些角色照樣看得到。
- 撤銷：嚴格模式（重構自動回滾）清理失敗回 `Err`、收據不彈出。重試時刪卡（檔不在＝成功）與清理都冪等；步驟 3a 插回的快照不含這次刪掉的 id（見二之 4），清理不會改到它們，重試時「已插回」辨識照樣成立，不會重複插。

## 四、測試
放 `data/worldbook/character_scrub_tests.rs`（清理規則）與 `receipts/character_delete_tests.rs`（掛點、收據轉換、撤銷）；不加長已過千行的檔。
1. 名單 `[A]` 刪 A → `"gm"`；`[A, B]` 刪 A → `[B]`；`[A, A]` 刪 A → `"gm"`；`[B]` 刪 A → 不變；一次刪 `[A, B]` → `"gm"`。
2. `"gm"`／`"public"`、讀不懂的名單（`["A", 42]`）、空名單 `[]` 都不動；`"public"` 帶 `source_cards: [A]` → 只移除欄位。
3. `source_cards`：`[A]` → 欄位移除；`[A, X]` → `[X]`；名單 `[A]`＋`source_cards [A, X]` → `"gm"`＋`[X]`。
4. 冪等：第二次跑回空清單、`worldbook.json` 逐位元組不變；第二次跑時注入寫入失敗也不報錯（證明沒寫）。收據同理：沒有可轉換的內容時不寫收據檔。
5. 原子寫：改名失敗（`RenameFailGuard::fail_ending`）→ 回 `Err`、`worldbook.json` 逐位元組不變、沒有殘留 `.tmp`。
6. 玩家刪角色：條目 `[A]`，刪 A → 讀成 `Gm`，GM 掃描下 `confidential()` 不成立（照 `import/foreign_ids_tests.rs` 用 `chat_assembly::test_gm_scan`）。
7. 判準：md 已刪、圖庫刪除失敗（注入移除失敗）→ 回原錯，但名單已清；md 刪除失敗 → 名單不動。轉條目路與撤銷非嚴格模式各一條同樣情境。md 存在檢查確實回 `Err`（拿掉 `characters/` 目錄的搜尋權限；這時刪卡看不到檔案而回成功）→ 回 `worldbook_cleanup_failed = true`、名單不動；只拿掉 md 的讀權不算，`try_exists` 照樣回 `Ok(true)`。
8. 清理失敗：注入世界書寫入失敗 → 卡已刪、回旗標、世界書與收據檔逐位元組不變；注入收據寫入失敗 → 回旗標、世界書已清、收據檔逐位元組不變。
9. 角色卡轉條目：被轉的角色 id 從其他條目拿掉；新條目照舊是 `Gm`；全程持獨占（用寫入鉤子在每次寫世界書的當下，另一條執行緒 `try_world_exclusive` 取不到、共用許可 150ms 內也取不到）。
10. 刪後撤銷：匯入卡 A（條目 `[A]`）→ 刪 A → 撤銷該筆，A 建的條目全數刪除、`kept_entries == 0`。對照：刪 A 前玩家改過其中一條內文 → 撤銷時那條保留、讀成 `Gm`。
11. 撤銷本身：匯入卡 A 後玩家把既有條目 E 手動加上 A → 撤銷 → E 名單不再含 A（只含 A 時變 `Gm`）；本次新建、玩家沒改的條目照舊被刪。
12. 收據轉換：二之 3 的七個情境各一條，都實際撤到底驗結果（公開升級；鏈式兩次撤銷不留孤兒；玩家改過的不可抵達情境兩次撤銷都保留 E；匯入 A → 重構刪除 E → 刪 A → 撤重構 → 撤匯入不留孤兒；同樣流程換成重構改寫 E；uid 碰撞時撤 R1 不刪 E′；玩家把 B 加進 E 後刪 A，撤 R2、撤 R1 不留孤兒）。另外：
    - 世界書沒有含 A 的條目、只有重構收據快照帶 A → 刪 A 後快照已清、收據有寫。
    - 舊收據（`deleted_entries_raw` 長度與 `deleted_entries` 不同）走精簡快照：轉換與撤銷結果正確。
    - 三筆交錯：R1 匯入 A → R2 重構改寫或刪除（各一條）→ R3 匯入同內容卡合併 → 刪 A → 逐筆撤到底，條目最後都不留孤兒、沒有誤刪。另一版在刪 A 前玩家改過內文：刪除分支那條一路保留到底；改寫分支撤 R2 時 `receipts.rs:875-879` 會無條件覆寫回改寫快照（既有行為不改），之後照原收據撤銷（撤 R1 時指紋相符就刪），測試照這個預期驗；「玩家修改保留」只在未被改寫快照覆寫的條目上驗。
13. 撤銷重試：重構收據帶 `deleted_entries_raw`，嚴格模式下清理成功、後段注入失敗 → 收據不彈出；再撤兩次，條目數與原始欄位（次要鍵、`source_cards`、`position`）跟只撤一次相同。
14. 兩條執行緒各刪一張卡，兩邊的世界書清理與收據指紋更新都在，沒有互蓋。
15. 收據原子寫：`write_receipts` 半截寫入／改名失敗 → 收據檔逐位元組不變。
16. 落定失效（接受，鎖現況），兩個獨立的初始狀態：
    - `Gm`＋`source_cards [A]`，刪 A → 欄位移除；再匯入同內容、沒寫可見度的卡 C → 收成 `[C]`。
    - 沒有來源卡的 `Gm`（同上刪 A 後的狀態，另建一桌）→ 匯入同內容、來源明寫 gm 的卡 → 仍是 `Gm` 並帶上該卡的 `source_cards`（重新落定）。
17. 前端（vitest）：`remove`／`removePlayer` 收到旗標時顯示提示、仍走刪除後善後，玩家卡善後後提示仍在；handler 回 `undefined` 不出錯；轉條目時提示接在 `convertCardDone` 後面。
- 驗證：`npm run verify`、`cargo test`、web vitest（網頁版沒改，確認仍綠）。
- 測試通道實測：匯入帶隨身世界書的角色卡 → 刪角色 → 世界書編輯器該條顯示 GM；角色卡轉條目同樣看一次。通道點不到原生確認框時停下問。

## 五、已知差異與限制
- 與 ST：ST 刪角色不動世界資訊（`characterFilter` 的名字照留）。本案照作者裁決清掉，邏輯簡單、比 ST 乾淨。
- 落定失效：`Gm` 條目的 `source_cards` 只剩被刪的卡時欄位會移除，條目變回「預設 Gm」，之後匯入同內容、沒寫可見度的角色卡會被收成那張卡的名單；重匯明寫 gm 的同卡仍會落定〔模型判斷·未裁決〕。
- 不是完全清除：只清這次刪掉的 id。桌上既有的別桌 id（worldbook-path-foreign-ids 的 P2，作者未拍板）、計時檔的 `char:<id>` 視角鍵都不處理〔模型判斷·未裁決〕。
- 收據轉換只沿撤銷鏈推可抵達的值；玩家在匯入之後手動改過的條目，比對照現行規則當成玩家改過而保留，不另外推測。
- 撤銷插回刪除快照時原 uid 被佔走、改插新 uid 的條目（`receipts.rs:873-879`），模擬仍以原 uid 推，較舊收據的指紋可能對不上而保留。
- 編輯器草稿：世界設定開著時，側欄隱藏區仍可刪角色；刪除後 `finishRemoval` → `setMainView(null)` 會卸載 `WorldEditor`，不會有舊草稿把名單寫回。這條路不經 `canLeaveEditor`、會丟未存草稿，屬範圍外，本案不做〔模型判斷·未裁決〕。
