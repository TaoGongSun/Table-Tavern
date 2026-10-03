# refactor-statusbar-skeleton — 狀態欄型的卡也產骨架

## 要做的事
重構判 playable: no（狀態欄型）的卡也產骨架：app 用每一樓的狀態快照，把原卡狀態區塊（例如 `<Status_block>` YAML）的格式填好，再交給原卡畫面顯示。模型只回報變動，不必每回合重寫整份狀態區塊〔作者裁決 2026-10-02〕。

## 接手須知
- **狀態：** 已結案（2026-10-03），squash 進 main；Sol 第 16 輪簽收（審查串 session `01a0fd2c-e699-71d2-80e5-4f8a2e69302c`）。驗證數字見 main 上本案 commit 訊息。
- **待實測：** 已排進 [實測佇列](../../reference/verification-queue.md) 第 19 項。
- **做法與規格：** [plans/refactor-statusbar-skeleton.md](../../plans/refactor-statusbar-skeleton.md)。
- **重放用產物（本機、不 commit）：** `/Users/pachelo/GitHub/Table-Tavern/TestCards/NorthHall-structure-statusbar-outcome-v2.json`，這輪 Sonnet 5.5 重構的產物（外框組殼、開場白初始值）。舊版 `NorthHall-structure-statusbar-outcome.json` 是裁決前的產物（兩份骨架、淺合併），只拿來把桌弄成「已重構」狀態。
- **重建測試桌（零額度）：** 一律在 test-harness 獨立 root 跑（`node scripts/harness.mjs launch --fresh --config-from <設定副本>`），不碰正式目錄。
  1. 開新的一桌，用 `TestCards/NorthHall-structure.png` 匯入成世界書，貼「開場白 1」。
  2. 世界設定 → 世界書 ⋯ → 匯入重構卡，選上面的 v2 產物，全部套用。
  3. 用零額度推進（不派 AI）產生 GM 樓，看各樓狀態欄。
- **零額度讀實際提示詞：**
  `TT_PROMPT_ROOT=<資料根目錄> TT_PROMPT_WORLD=<桌 id> TT_PROMPT_OUT=<輸出檔> cargo test --lib dump_gm_lane_prompt -- --ignored`
  入口在 `commands/chat.rs`，走正式的 `gm_materials`＋`gm_turn_instruction`＋lane 組裝，不打 AI。

## 目前程式的行為（重點）
- **展開分類：** `EntryKind` 分 `InterfaceShell`（yes）與 `InterfaceStatusbar`（no，含 SPLITS 的 statusbar 段），兩者都產 STATE＋SHELL＋RULES＋GUIDE。解析器要求四塊完整，佔位符要對到 STATE 葉子與規則，不完整就該條失敗。
- **保留來源：** 產物帶 `preserve_source_uids`，套用時不刪、不停用這些來源。範圍包括：
  - 沒成功完成的展開任務：失敗、取消時正在跑、取消後沒發出。
  - 介面合併衝突的全部候選來源。
  - 匯入重構卡時會讀回這份清單。
- **合併：** 依最小來源 uid 穩定排序；狀態樹遞迴合併；值或規則衝突、合併後佔位符對不上，都算衝突，整組介面不套、列入失敗清單。
- **佔位符契約：** 前後端共用 `src/shared/contracts/st-macros.json`。分四類：
  - 正文槽 `{{本回合.正文}}`。
  - STATE 有這個葉子就是狀態引用，優先於同名巨集。
  - 已知酒館巨集原樣保留，帶參巨集只認已知名稱。
  - 其餘一律當路徑，含未知的冒號寫法。
- **YAML 填值：**
  - **範圍：** 只在「解析 YAML 的那支卡腳本」的容器標籤內，而且容器內文真的是 YAML 結構（可略過註解與 `---`）。正文槽永遠照原樣。
  - **寫法：** 依雙引號、單引號、區塊純量、一般純量、行內集合的上下文寫值。行內集合逐個元素表示，契約見下面「行內集合契約」。
  - **型別表 `mechanism.value_types`：** 套用時記下。number／bool 取自**模型產出的 STATE 初始 JSON** 葉子型別（不是原卡 YAML 本身）；list 取自同一份產物的欄位規則 `kind: list`。number／bool 欄的值是合法字面就照原樣寫，卡讀回數字／布林；其餘會被 YAML 轉型的值（日期、科學記號、null／bool 字面等）一律加引號，讀回字串。
    - 待驗：原卡數值／布林欄展開後，模型有沒有把它們寫成字串（寫成字串就不會記成 number／bool，卡讀回字串）。NorthHall 產物的糧草、馬匹兩欄是數字，其他卡未驗。
  - **行內集合契約：** 每個含佔位符的元素都以完整元素表示。
    - **集合片段：** 只有 `[{{x}}]` 唯一內容、而且 x 是 list 欄才算。值本身是明確的 YAML 序列（`[a, b]` 或 `- a` 區塊清單）就取它的元素；普通多行文字是一個字串元素、保留換行；單行值照 YAML 行內序列解析（`"a, b", c` 兩個元素；全形「甲，乙」不是分隔符號，一個元素）。片段用 js-yaml core schema 解析（沒有合併鍵與日期）；各元素保留型別、逐個寫成 JSON，寫完重新解析核對相同才採用。解析不了、有循環或過大的 alias、不是 JSON 型別（例如 .inf）、核對不符的，整份原值當一個字串元素。
    - **單一元素：** 其餘單一佔位符遵守型別表，含 `,[]{}` 或換行就整個加引號。
    - **前後綴元素（`[前綴{{x}}]`）：** 整個元素填好後當字串表示，換行與指示字元原樣讀回；原卡巨集 token 在雙引號裡原樣保留（`"{{user}}前綴…"`）。目前合成顯示流程（填骨架→卡片 regex→沙盒快照→卡片腳本解析）不自動替換巨集；日後若加入巨集替換，須在值替換完成後再做 YAML 跳脫。
    - **引號元素：** 雙引號照跳脫寫入；單引號元素內含換行時改寫成雙引號純量。
- **每一樓各自合成：** 旁白／角色對話樓用該樓的 `state.tree`＋text 填骨架；原文自己畫得出殼的樓用原文；玩家與 system 樓不合成。選殼先試最新 GM 樓，再走十樓 fallback。
- **回合尾三態** `gm_turn_format(有顯示腳本, 有介面骨架, mode)`：characters 桌與沒有腳本的桌走一般旁白（＋```state 圍欄）；有骨架的桌走接管指示 `takeover_instruction`（正文後輸出 `<UpdateVariable>`，只寫有變動的欄位，沒變動就不寫，不要 ```state、不重印狀態區塊）；有腳本、沒有骨架的桌照卡片格式。前端用同一依據（骨架非空、不是 characters 桌）的 `interfaceTakeover` 不顯示頂部狀態欄。
- **本桌更新範例：** 接管桌的 system 在本桌 guide 之後、介面歸屬聲明之前，附一段用本桌欄位規則真實路徑寫的 `<UpdateVariable>` 範例（delta、replace 各一筆，取規則表排序最前），並註明不要照抄通用協定的 `/Heroes`、`/World` 範例。通用協定原文不動。
- **present 缺席：** 接管桌沒有在場名單。只有介面接管桌（骨架非空、不是 characters 桌，與 `gm_turn_format` 同一依據）在 present 缺席時換幕不結算角色卡隱藏，否則原卡主角色會被收進隱藏區、從 GM 上下文消失〔模型判斷·未裁決〕。characters 桌與沒重構的桌照原行為。人物登場與角色卡回歸在 present 是 None 時退回正文比對，介面桌的 PERSONS 本來就被清空，無害。
- **外框條目：** 盤點後，程式判定外框候選（`refactor_ai/frame.rs`），規則刻意嚴格，寧可漏判、走一般展開：
  - 至少一個 `<Tag>…</Tag>` 容器；
  - 每個容器去掉 `${…}`、`{{…}}` 後只剩空白，含任何固定文字（例如 `糧草＝300石`）就不算；
  - 容器外沒有佔位符，也沒有縮排的「鍵: 值」欄位定義（規則條列 `- …` 與頂層標題不算）；
  - 前端分兩階段派發（`refactor-frame.ts`）：定義條目先展開；有定義骨架含相同容器標籤的候選才當外框，零 AI，不產 STATE／RULES／GUIDE；其餘照常展開。合併時照外框的容器順序組殼：對上的容器搬入，唯一沒對上的容器放正文槽。組不起來就記失敗、保留來源。
- **開場白初始值：** 玩家貼出的那則開場白（取自匯入原檔＋記下的序號，不讀逐字稿）裡，與條目同標籤的容器區塊會帶進 expand 的 user 訊息。提示詞要求初始值以它為準，欄位要能原樣表示這些值。
- **匯入原檔（`import/files.rs`、`receipts/sources.rs`）：** 兩條匯入路徑依序：
  1. 先驗卡檔，壞檔直接拒絕，不留任何東西。
  2. 寫自己的 `import-pending-<id>` 未完成標記，寫不進去整次不做。
  3. 另存 `import-source-<新 id>.<png|json>`。
  4. 匯入。
  5. 記收據（原檔識別：路徑、匯入名、角色色、檔名掛在收據上）。
  6. 原檔與收據都成功才刪標記。
  
  其他規則：
  - 匯入結果回傳這次匯入的識別 `source`（什麼都沒新增、沒留收據時是 null）。前端在開場白面板記著，貼開場白時連同 `opening_index` 帶回 post_opening，序號只掛到那筆匯入的收據。帶了序號卻歸屬不了（例如重複匯入沒留收據），標記不解除。
  - 貼開場白一樣先寫標記，記好才刪。逐字稿是直接 append，失敗時可能已留下半行：貼之前先拍這一幕逐字稿與 state.json 的原始位元組（`data::opening_checkpoint`），失敗就寫回、讀回確認原樣才解除標記；回不去（或貼前就讀不到）標記留著。
  - **整桌獨占：** 匯入（兩條路徑）、`undo_last_import`、貼開場白、`refactor_apply`、改名補記（`record_import_rename`）的 command 改成 async，整段持整桌獨占（`data::world_exclusive_async`：tokio RwLock 排隊等在途寫入放開，排著時擋住後到的共用許可；閘門開了回錯）。快照、資料變更、記帳與失敗復原之間不會插進聊天等共用許可的寫入。資料函式（`import_worldbook_file`、`import_character_file`、`post_opening_text`、`record_posted_opening`、`undo_last_import`、`record_last_import_rename`、`record_refactor_apply`）要出示 `&WorldExclusive` 參數，編譯期保證呼叫端持鎖；重設在臨時根重匯沿用自己那張獨占。所有收據寫入都在獨占內，第 10 輪加的收據互斥已刪。測試直接呼叫時用 `data::test_exclusive`。
  - 撤銷匯入時收據彈出、原檔一起刪。
  - `source-card.*` 維持原意。
- **來源不完整怎麼辨識：** 以下任一情況都算，`refactor_rerun_status` 回 `no_source`、重設擋下：
  - 桌目錄還有任何 `import-pending-*` 標記：匯入或貼開場白中途失敗、收據讀壞被重寫、收據寫不進去、序號歸屬不了。
  - 收據讀不了。
  - 有角色卡或世界書匯入收據沒掛原檔，例如這套做法之前匯入的舊桌。
  - 原檔讀不到。
- **重新重構：** `refactor_rerun_status` 分四態：未重構照現狀；已重構且已遊玩（current_scene>0 或逐字稿超過 1 則）擋下；沒有匯入原檔或來源不完整擋下；其餘確認後 `refactor_reset_to_source`。`refactor_outcome_exists` 已刪。
- **重設的鎖：** `reset_to_import_source` 整段持這桌的獨占鎖（`try_world_exclusive`），拿到鎖才檢查 Ready。有在途寫入或另一次重設就回 WorldBusy；重設進行中，新的寫入許可排隊等它放開。command 只保留更新閘門檢查，不另拿共用許可。
- **重設怎麼做、失敗怎麼復原：**
  - 先在隱藏的臨時資料根 `worlds/.tt-reset-<id>/worlds/<id>` 建一張同 id 的空白桌（桌名、模型指定沿用）。照收據順序用原檔重匯，每筆匯入的開場白（同原文、同序號）緊接著重貼。沒掛在收據上的開場白也照原文貼回。
  - 交換前驗重建桌：沒有未完成標記、收據讀得到，而且重放出來的來源（路徑、名、色、原檔 bytes）與開場（場景、時間戳、序號）跟預期逐筆一致，整桌也讀得完整。任何不符禁止交換。
  - 驗過才用格式機制的 `reset` 操作交換：重建桌搬成 `.tt-staging-<id>`、寫 `.tt-op-<id>.json`、原桌搬成 `.tt-trash-<id>`、staging 上位、刪 trash 與日誌。
  - 交換前任何一步失敗（原檔壞、某筆重匯失敗、改名失敗），臨時根與 staging 一律刪掉，原桌一個位元都不動，回錯給前端。
  - 交換途中當機：下次開桌照恢復表處理。原桌還在就丟掉 staging；原桌已搬走時，staging 驗過就換上，驗不過就把原桌搬回；只差清理就刪 trash。
  - 交換成功但臨時根清不掉：回 `committed_cleanup_pending`，前端一樣當成功；殘留在下次開桌（`open_world` 持獨占時）、下次重設或刪桌時清掉，桌清單看不到它。三處都走 `data::remove_reset_build_root`。
- **清殼規則保留：** 「套用時沒有新殼就清掉舊殼」在按重構的路徑已不會遇到（先清回原卡），但「匯入重構卡」仍可能套到已重構的桌，規則不是死碼，保留。
- **收據：** `guide_before` 與 `value_types_before`（整份原表，含原本的空表），undo 都寫回；`Mechanism::is_empty` 把 guide 與 value_types 算進去。

## GUI／AI 實測（2026-10-03，test-harness 獨立 root，正式目錄 hash 前後相同、含金鑰的設定副本已刪）
- **清回原卡：** 匯入 NorthHall-structure（世界書路徑，當時的版本把開場白序號記成 0；之後第 8、9 輪改了存法，見上），零額度匯入舊產物變成已重構。按重構跳出「會用原卡重新重構」確認，確認後殼、產物、guide、型別表、增量全部清掉，世界書回到 23 條，逐字稿只剩開場白。
- **Sonnet 5.5 重構一次**（初判＋盤點＋4 個展開，共 6 次呼叫）：
  - 第 22 條判為外框，沒有送展開。
  - 骨架只有一份：`<maintext>{{本回合.正文}}</maintext>`＋一個 `<Status_block>`。來源 21、22 一起消耗，沒有衝突，guide 一份。
  - 初始值等於開場白：前院、酉时；糧草設計成文字欄「账载四百四十石 实存待查」；在場三人；四個行動選項。
  - 初判建議改成多角色，這次手選保留原卡玩法。
- **Haiku 4.5 一回合**（1 次呼叫）：
  - 正文後有 `<UpdateVariable>`，路徑全是本桌的（/在場人物、/地點、/糧草），狀態樹實際變了（在場人物）。
  - 沒有 ```state，沒重印狀態區塊，頂部狀態欄不顯示。
  - 同一回合有兩筆沒套上：/地點 的 JSON 字串尾巴多了跳脫引號，被容錯解析跳過；/糧草 對文字欄下了 delta，被規則擋。屬於模型輸出品質，不是契約互斥。
- **已遊玩桌按重構：** 跳出「這桌已有遊玩資料…請開新桌重構」，沒有 AI 呼叫，桌不動。

## Sol 驗收第 8 輪（不簽收；必改 1～5 已修）
- Sol 確認三態、頂欄隱藏、路徑範例、開場白初值、失敗外框保留來源都符合裁決。
- 未通過的必改與修法：
  1. 重設先清原桌，失敗會留空桌。改成臨時根重建後原子交換，補壞角色原檔、第二筆匯入失敗、交換寫入失敗與四種當機組合的測試。
  2. 原檔保存失敗被吞。改成先存原檔、存不進去就不匯入；收據與開場序號記不下來時標記來源不完整，補部分保存失敗測試。
  3. 撤銷匯入沒同步來源清單。來源改掛在收據上，撤銷時一起刪；檔名改用新 id，補「撤銷 B 後重設」測試。
  4. 外框判定放過固定文字。改成上述嚴格規則，補「同容器但含固定文字」與容器外欄位定義的測試。
  5. present 修正波及其他桌。改成只限介面接管桌，補 characters 與一般桌的回歸測試。
- 這輪沒跑 AI，也沒跑 GUI。上一輪 GUI 驗的流程，這輪只改了內部存法與失敗處理，單元測試已涵蓋。

## Sol 驗收第 9 輪（不簽收；必改 1～4＋建議 2 項已修）
- Sol 確認 `.tt-reset-*` 不會被桌列表誤認，reset 恢復與 migrate／restore 分開，外框、撤銷、present 的修正都符合。
- 修法：
  1. 重設改持整桌獨占鎖，拿到鎖才檢查 Ready。測試：有在途寫入或另一次重設時擋下、原桌不動；重設持鎖時新寫入排隊。
  2. 不完整標記改成每次操作專屬的 pending 標記，資料變動前寫、完成才刪。測試：角色匯入寫到一半失敗（角色資料夾唯讀）會判不完整；壞卡檔不留標記。
  3. 交換前逐筆比對重建桌的來源與開場，並檢查未完成標記。測試：開場白在逐字稿消失時，重放清單對不上，禁止交換、原桌不動。
  4. 開場白綁定匯入識別。測試：A、B 世界書條目相同、開場不同時，B 的開場不會掛到 A，判不完整。
  - 建議 1：`RenameFailGuard::fail_after(skip, times)`。測試：原桌搬走那步失敗；staging 上位失敗、退回成功；兩者都失敗、下次開桌換上重建桌。
  - 建議 2：交換後臨時根清不掉時回 `committed_cleanup_pending`，開桌時順手清（故障注入測試在第 10 輪補上）。

## Sol 驗收第 10 輪（不簽收；必改 1～2＋建議 2 項已修）
1. 收據寫入並行遺失：當時改成收據互斥；第 11 輪再改成整桌獨占（見下）。
2. 貼開場白失敗直接解除標記：改成回復確認後才解除（見上）。測試：`AppendFailGuard::partial` 只追加半行就失敗，逐字稿與狀態位元組回原樣、標記解除；原本沒有逐字稿且刪不掉半行（`RemoveFailGuard::fail_ending`）時標記留著、判不完整。
- 建議：`committed_cleanup_pending` 故障注入（`RemoveFailGuard::fail_ending(".tt-reset-<id>")`）：回 cleanup_pending、桌已換成重建桌、桌清單只有本桌，下次開桌清掉殘留；前端照成功刷新再接著重構（`refactor-workflow.test.tsx`）。
- 建議：前端來源識別傳送測試（`src/features/import/opening-source.test.tsx`）：匯入回傳的 source 照 App 接線原樣送到 `post_opening.importSource`，null 時送 null、不沿用上一次。

## Sol 驗收歷程
第 4～6 輪的必改已併入上面的行為描述（型別表進收據、集合片段契約、js-yaml 4.3.2、前後綴與巨集元素）。第 7 輪簽收實作，收尾只改巨集註解。

## Sol 驗收第 11 輪（不簽收；必改 1～2 已修，主線裁示做法）
- Sol 確認收據互斥、非並行的開場復原、兩項建議測試都已落實。
- 必改的根因相同：匯入、undo、貼開場只持共用許可，資料變更與記帳之間可被其他寫入插入。
  1. 開場復原覆蓋並行寫入：A 拍回復點後 B 貼訊息，A 失敗整檔寫回會刪掉 B。
  2. undo 只鎖收據：B 讀到含 A 的世界書，undo 刪 A 後 B 寫回，A 復活卻沒有收據。
- 修法：改成整桌獨占（見上「匯入原檔」），`refactor_apply` 與改名補記一併納入（同樣是快照→變更→記帳，留著共用許可下一輪必然再被點）〔模型判斷·未裁決〕。
- 測試（`receipts/tests/race.rs`）在資料寫入點交錯（`data::write_hook`，指定路徑寫入前停住）：
  - 貼 A 開場停在逐字稿追加前，B 的聊天寫入（共用許可）被擋到 A 完成，逐字稿是 A 再 B。
  - 同上但 A 只追加半行就失敗：逐字稿是「前情、B」，狀態是 B 寫完那刻，A 標記解除。
  - 撤銷 A 停在世界書寫入前，匯入 B 被擋：世界書只剩乙、收據只剩乙、A 原檔已刪。
  - 測試取鎖改成鎖到別的桌時三條都紅（已手動驗過）。`world_lock` 另補排隊獨占擋後到許可、閘門開了回錯兩條。
- 行為變化：貼開場白、匯入在 AI 生成中（聊天持共用許可）會等生成結束才執行，不回忙碌。

## 第 12 輪（Sol 可簽收；結案前補兩項建議）
- 排隊回饋（`src/features/play/useTurnWait.ts`）：`run` 讓整段操作同時只跑一次（等待中再按忽略），`backend` 包住那次後端呼叫，送出當下 `chat.isBusy()` 為真就亮 waiting、回來就收。提示字 `turnQueuedWait`（十語系）放在操作當下的位置：
  - 開場白面板：「貼出」鈕就地換字（SwapLabel），兩顆貼出鈕停用。
  - 重構結果卡：「全部套用」與明細的「套用」鈕就地換字（原本 busy 已停用）。
  - 匯入、撤銷匯入：從陣容欄「＋」選單發起，選單會收起，提示放在「＋」上方（寬欄文字、窄欄 ⏳＋title）。撤銷只在還沒開演時出現，實際上碰不到生成中，仍接同一套防重複。
  - 停止生成的入口沒動。

## 第 13 輪（必改：排隊中的舊操作換桌後回寫）
- `useTurnWait(isTurnRunning, scope)` 以目前桌為 scope：`run` 給操作 `live()` 與 `backend`。換桌或卸載後舊操作 `live()` 為 false、不擋新桌；已送出的後端操作照常在原桌完成，完成回呼先問 `live()`，不算數就不回寫畫面、不刷新、不關面板、不報錯。
  - 貼開場白：協調抽成 `src/features/import/useOpeningPost.ts`；`chat.postOpening` 收 `isCurrent`，不算數就不 setEvents／不刷新；面板世代（`imports.openingsPanelId()`）對不上不關。
  - 撤銷匯入（App）、套用重構（`useRefactorWorkflow`）同樣先問 `live()`。
  - 匯入在 `runTableOp` 換桌互斥內，途中還會自己換到新桌，只用不分桌的 `backend` 亮提示。
- 測試：hook 換桌與卸載兩條；貼開場白「A 桌排隊中換到 B 並開了 B 的面板，A 回來不動 B 的畫面與面板」與卸載；套用重構換桌與卸載。拿掉 `useOpeningPost` 的檢查時換桌那條會紅。撤銷沒有單獨的換桌測試（只靠 hook 測試）。
- GUI 實測（2026-10-03，test-harness 獨立 root，claude CLI 的 claude-sonnet-5-5 一回合，作者授權；正式目錄 1453 檔 hash 前後相同，測試設定不含金鑰）：送玩家訊息後生成中打開世界設定、零額度匯入 `NorthHall-structure-statusbar-outcome-v2.json`、按「全部套用」→ 按鈕就地換成「等目前的回覆結束後執行…」並停用（其餘鈕一併停用）；再點回 disabled、JS 直接 click 也沒送出，生成中只有原本那筆世界書收據；生成結束後套用照常完成（重構完成訊息、收據只多一筆 refactor）。截圖在主線 scratchpad `refactor-apply-waiting.png`。
  - 這次觀察到排隊的套用在 AI 串流結束、後端放開許可時就執行，早於前端寫 GM 旁白；第 14 輪已改。
  - 沒驗：停止生成（世界設定畫面蓋住聊天區，停止鈕在聊天區）；貼開場白（開場白面板是 modal，正常點擊無法在面板開著時開始生成）。
- `record_posted_opening` 也要出示 `&WorldExclusive`。

## 第 14 輪（必改：排隊操作要等前端整個回合落檔）
- `useTurnWait` 的 `backend` 先輪詢（50ms）等 `isTurnRunning()`（`chat.isBusy`，GM 正文、狀態事件、角色回覆落檔與刷新跑完 finally 才清）為 false 才送後端；gmAdvance 接力、停止生成後的截斷落檔都涵蓋在 busy 內。不先拿後端獨占再等，後端獨占仍是最後防線。等待期間換桌／卸載（不分桌版只看卸載）丟 `TurnWaitAborted`、不送出，`run` 收掉回 undefined。等待提示照舊亮到後端回來。
- 撤銷匯入（App）每個 await 邊界都重問 `live()`；套用重構在 `refreshAfterApply` 後再問才跳完成訊息。
- `characters.refresh()` 加世代號 `castLoad`：換桌（hydrate）或又重讀後，晚到的舊桌陣容不寫進畫面（撤銷途中換桌時它原本會把舊桌陣容蓋到新桌）。
- 測試：hook 的回合結束前不送、等待中換桌／卸載不送；開場白接真 `useChatController`，串流結束後 append_transcript 懸著時不送 post_opening、回合 finally 後才送，等待中換桌不送；套用等待中換桌不送、刷新中換桌不跳訊息；App 層撤銷在 `characters.refresh()` 懸著時換桌，不改新桌發言對象、不刷新舊桌殼／逐字稿／收據、不重讀桌清單。拿掉等待、撤銷檢查、套用檢查三處時對應測試都會紅（已手動驗過）。
- 這輪沒跑 GUI。Sol 第 15 輪確認整回合等待、50ms 輪詢、castLoad、兩回合之間執行排隊操作都可接受。

## 第 15 輪（必改：刷新鏈內部的換桌防護）
- 套用刷新鏈：`refreshAfterApply(live)` 經 WorldEditor 一路傳到 `App.refreshAfterRefactorApplied(live)`，每個 await 後、改畫面前都檢查；清回原卡那條不排隊的路徑用 mounted＋桌別自組 live。
- `useCardInterfaceController` 的介面腳本與骨架各加世代號（`interfacesLoad`、`shellLoad`，切桌 effect 與 `refreshInterfaces`／`refreshShell` 共用），晚到的舊回應不寫進畫面；`imports.refreshReceipts` 讀回來時桌別不符就不寫。
- 測試（`refactor-undo-flow.test.tsx`）：跑實際 App 刷新鏈，撤銷匯入與套用重構各三種懸置點（characters.refresh、refreshInterfaces、refreshShell）懸著時換桌，驗新桌發言對象、陣容、介面鈕、頂部狀態欄（骨架接管會收掉）都不被舊桌覆寫，放行後也不再碰舊桌。拿掉 loader 世代號、castLoad、套用鏈檢查、撤銷鏈檢查時對應測試都會紅（已手動驗過）。

## 定案結論（原待問 1～4）
1. **重跑重構**〔作者裁決 2026-10-03〕：已遊玩的桌擋下並提示開新桌；未遊玩但已重構的桌確認後用原卡清回、再照一般流程重構，永不拿重構後的資料再重構；沒有原卡檔的舊桌擋下；未重構的桌照現狀。原卡檔做法（另存 `import-source.*`）、清回時保留哪些東西、「未遊玩」的定義〔模型判斷·未裁決，主線同意〕。
2. **Haiku 抄通用範例路徑**〔作者裁決 2026-10-03〕：本桌 guide 附近附本桌真實路徑的範例，通用協定原文不動。
3. **同一狀態欄兩條來源**〔作者裁決 2026-10-03〕：只有真正定義欄位的條目產 STATE／RULES／GUIDE，外框條目只貢獻外框；初始值以開場白實際寫的容器區塊為準，開場白沒有才用定義條目的值；兩條都定義同一欄位且值不同仍照衝突規則整組不套。外框用程式判定、要有對應的定義骨架才算〔模型判斷·未裁決，主線同意〕。
4. **接管桌輸出契約**〔作者裁決 2026-10-03〕：有介面骨架的桌不顯示頂部狀態欄，回合尾改成正文＋只寫變動的 `<UpdateVariable>`，與 system 的增量協定一致；沒重構的桌、characters 桌照舊；判定沿用骨架＋mode。

## 下一步
結案。剩餘只有實測佇列第 19 項的實機觀察與待查證（酒館 regex 替換字串與訊息顯示會不會替換 `{{user}}`／`{{char}}`〔模型判斷·未裁決〕；酒館會替換的話面板也應替換，只查規格寫結論）。
