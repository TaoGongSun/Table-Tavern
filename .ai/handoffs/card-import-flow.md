# Handoff: card-import-flow

## Current state
四包全部實作完成、主線逐包複驗綠、逐包 commit。2026-08-05 一輪實機驗收挖出四件並全數修完、逐項通過（身分框文案、腳本提示、開場白按鈕位置、兩路徑對等化）。驗收七項通過五項，剩 4、7 卡在沒有合適樣本卡。
2026-08-06 追加兩件（樣本卡實測驅動）：雙世界書由封鎖改成可融合（已驗收）；身分判定改版（進行中，見 Remaining 的三步）。

## Completed
- 包 1：世界書匯入（兩路徑）後一律 setSpeaker(GM)；匯入成功後自動名桌改成卡名（probe 新增 name；純世界書檔退用檔名）；去重不再觸發選 GM。
- 包 2：匯入收據＋一鍵復原。[receipts.rs](../../src-tauri/src/receipts.rs)（快照 diff；空匯入不留收據；損毀收據回錯不裝沒事；機制／擴充／世界卡殼／桌名全蓋）；指令 list_import_receipts／undo_last_import／record_import_rename；前端「復原上次匯入」（收據非空才顯示）＋復原後全面刷新＋speaker 落空改 GM；i18n 十語系 5 鍵。
- 包 3：單一入口三分流（純世界書零詢問直匯、純角色卡零詢問直匯、兩者有料跳三鍵框用 lorebook_heavy 預選）；舊兩鍵改道框刪除；WorldEditor 匯入按鈕與 onImported／onOpening 整組移除（保留新增條目／去重／匯出）；probe 加 parsed／book_entries／book_shaped（book_shaped 為主線驗收補修：V2 獨立書 JSON 自帶書名不被誤判成角色卡）。
- 包 4：第二張卡路由（[import-routing.ts](../../src/import-routing.ts) 純函式：direct／companion／block_double_worldbook／ask）；路由框「開新桌並匯入（主）／匯進這桌／取消」，雙世界書版無「匯進這桌」；開新桌並匯入＝create_world(label)→顯式 worldId 匯入→enterTable，原桌不動；importAsCharacter 等五函式參數化 worldId。

- 身分框文案改寫（2026-08-05 實機驗收後）：舊的共用文案「這張卡角色與世界書都有料…」語氣像在推薦匯入成角色卡，世界書卡照做會玩不動。改成偵測到哪一種就只講那一種——`importChoiceCharacterTitle/Body`（這張卡看起來是角色卡／要匯入成角色卡嗎？匯入成世界書可能無法遊玩。）與 `importChoiceBookTitle/Body`（世界書版對調），舊 `importChoiceTitle/Body` 兩鍵刪除，按鈕改「匯入成角色卡／匯入成世界書」；十語系同步，App.tsx 依 lorebook_heavy 選字串。

- 兩句「已只讀入…」腳本提示整組刪除（世界書路徑與角色卡路徑各一句，十語系鍵一併移除）。連帶清掉死線路：importAsCharacter／importAsWorldbook 的 probe 參數、路由框狀態與 routeImport 的 probe 欄位、後端 ImportProbe 的 `scripts`／`alternate_greetings` 兩個欄位與偵測程式（改由機制帳本負責交代）。查證：條目全部原樣存進 worldbook.json（只改欄位名對應、配 uid、預設 GM 可見），`<%` 原文也留著；不執行、不送模型的那些都在機制帳本有記（Absorbed／Skipped），唯一真的被丟的是指紋重複的條目，另有「N 條重複跳過」提示。舊提示只要檔案出現 `<%` 就跳，連已成功轉成觸發表的卡也跳，字面又像在說內容被捨棄。
- 開場白選擇框的「貼出這條」搬到框外左下（原本跟在全文後面，長開場白要整段捲到底才按得到）；`footer-lead` class 靠 margin-right:auto 推左，右下維持「先不要」。展開哪一條就貼哪一條，沒展開時不顯示。（承自已結案的 st-ecosystem-upgrades 第六項，實機驗收後的修正）
- 世界書路徑補上「介面畫不出來」的說明：雲端載入器卡／加密卡以世界書身分匯入時原本一句話都沒有，玩家只看到滿螢幕原始標記又找不到介面按鈕。兩條路徑共用 `unsupportedInterfaceNotice()`（世界書的介面殼 character_id 是空字串）。實例：訓帝卡唯一啟用的顯示腳本是 `$('body').load('https://…github.io/…')`，介面在作者網站上，不抓不執行＝沒有介面可開；西幻卡的腳本自帶完整 HTML 才畫得出來。
- **兩條匯入路徑對等化**（使用者立規：功能禁止只做角色卡那條路）。逐項對過後補三處：(A) 後端 `import_card_extension()` 讓世界書路徑也收本 app 匯出的 `extensions.table_tavern`（欄位規則＋初始狀態樹），原本只有 import_character 內部會套；(B) 前端 `tellAboutInterface()` 兩路共用，世界書路徑也會講「這張卡自帶介面，點上方卡片介面打開」；(C) `import_character` 指令改回傳 `{meta, book}`，角色卡隨身的世界書條目也報「已收進 N 條（重複跳過 M 條）」，訊息與世界書路徑同一個組字函式。刻意單邊的只剩開場白詢問（角色卡的開場白在卡上，不該由 GM 再貼一次，已在註解寫明）。

- **雙世界書改成可融合**（2026-08-06 拍板並驗收，推翻拍板 4 的「硬擋」）：路由框第二本世界書也給三顆鈕，中間那顆叫「仍要匯入」。資料層本來就支援——`data::import_worldbook` 讀既有書、新條目配新 uid 接在後面、指紋重複自動略過；收據先拍既有 uid 快照只記自己新增那批，所以兩本書各一筆收據、退第二本不誤傷第一本。路由名 `block_double_worldbook`→`merge_worldbook`、i18n 鍵 `importRouteBlock*`→`importRouteMerge*`，新增 `importRouteMergeAnyway`（十語系），文案改成「再匯一份會跟原本那份合成同一本；一模一樣的條目會自動略過。要分開放就開新桌」。

## Verification
- 最終四件套（2026-08-05 主線複驗）：cargo test 327 綠、vitest 22 綠（含 import-routing 5 例）、npm run build ✓、check:i18n 十語系 82 鈕 OK。
- 文案改寫自驗（2026-08-05）：npm run build ✓、vitest 22 綠、check:i18n 九語系 OK（按鈕加長後仍在寬度上限內）；接線見 [App.tsx:5546](../../src/App.tsx:5546)。
- 刪腳本提示自驗（2026-08-05）：cargo test 326 綠（原 327，刪掉兩個只驗 scripts 欄位的測試、新增 probe_ignores_invalid_bytes 保留格式錯誤那條斷言）、npm run build ✓、vitest 22 綠、check:i18n 九語系 OK；殘留檢查 grep importScriptNotice／worldbookScriptNotice = 0。淨變化 −75 行。
- 開場白按鈕改位自驗＋使用者實機確認通過（2026-08-05）：npm run build ✓、vitest 22 綠、check:i18n 九語系 OK。
- 世界書路徑說明自驗（2026-08-05）：npm run build ✓、vitest 22 綠、check:i18n 九語系 OK。原始卡檔實地拆解見 history 當日紀錄。
- 路徑對等化：使用者實機測試通過（2026-08-05）。自驗：cargo test 326 綠（既有 round-trip 測試延伸驗「同一份匯出檔改用世界書身分匯入，機制照樣收進來」）、npm run build ✓、vitest 22 綠、check:i18n 九語系 OK。
- 實機驗收第 6 項通過（2026-08-05 使用者）：新開的桌匯過世界書後再匯世界書會被擋。舊桌（收據功能之前建立的）沒有 `import-receipts.json` 擋不住——使用者拍板**不補**，新功能不覆蓋舊桌可接受。
- 雙世界書改可融合自驗（2026-08-06）：npm run build ✓、vitest 22 綠、check:i18n 十語系 81 鈕 OK；殘留檢查 grep `importRouteBlock|block_double_worldbook` = 0；淨變化 +75 −42 行（13 檔）。**使用者實機驗收通過**（2026-08-06）。
- 逐包 commit：548905a（包 1）、1939af6（包 2）、d8abe8b（包 3）、包 4 見 b4acab7。
- 主線抽讀確認：decideImportRoute 決策表逐條符合拍板；路由框 block 版 JSX 只有取消＋開新桌（App.tsx 搜 -a "importRoute !== null"）；openNewTableAndImport 的 worldId 全程來自 create_world 回傳值。

## Remaining
使用者實機驗收清單（2026-08-05 一輪驗完，通過的畫線）：
1. ~~匯帶書的角色卡 → 選角色卡：角色上桌、附帶條目進世界書、桌名變卡名~~ 通過
2. ~~純世界書 JSON 丟側欄「匯入卡」→ 不問直接進、目標指 GM、桌名用書名／檔名~~ 通過
3. ~~GM 編輯世界書區只剩「新增條目／清理重複／匯出世界書」~~ 通過
4. **保留（缺素材）**：「復原上次匯入」連按逐筆退——手邊沒有足夠的卡可以疊出多筆匯入紀錄。單筆退整筆（角色＋條目＋桌名）與「改過的條目保留並提示保留數」也一併等素材。
5. ~~已有卡的桌再匯卡 → 路由框；選「開新桌並匯入」→ 新桌名＝卡名、原桌不動~~ 通過
6. ~~已匯過世界書的桌再匯世界書 → 只有「開新桌並匯入／取消」~~ 通過（限新桌）
7. **保留（缺素材）**：有角色卡的桌補匯配套世界書 JSON → 不跳框直接進。手邊沒有成對的角色卡＋世界書。

### 身分判定改版（2026-08-06 拍板，三步依序做）
起因：`main_furry-male-scenarios-*_spec_v2.png` 兩張卡內容全是世界書，作者卻塞在 `description`（1873／1886 字），`character_book` 是空的。現行分流 [App.tsx:4076](../../src/App.tsx:4076)「有 name＋零條目＝純角色卡」直接匯，玩家連選都沒得選；卡的價值九成在 29／10 個 `alternate_greetings`，全被鎖在一張沒人格的角色卡背後（開場白選單只在世界書路徑跑）。

樣本統計（TestCards 22 檔）定的判準：`first_mes` 18／18 全有值（6–4359 字，世界書卡也都有）＝零鑑別力，**不可用**；`alternate_greetings` 只 4 張有（29／10／3／1），後兩張本來就被 `lorebook_heavy` 判成世界書，**樣本上零誤判，不設門檻**。

1. **接「人設欄→條目」轉換路**（先做，否則第 2 步的主按鈕按下去會爆）：[import.rs:1305](../../src-tauri/src/import.rs:1305) `worldbook_json` 只剝 `character_book`，沒那層時 fallback 成整包卡 JSON，`data::import_worldbook` 找不到 `entries` 報錯。改成沒有條目時把非空人設欄合成一條沒關鍵字的常駐條目（`constant: true`，同匯出方向 [import.rs:1020](../../src-tauri/src/import.rs:1020) 的做法）。**有條目的卡維持原樣不動**（避免回歸既有驗收，人設欄是否也該收另案）。
2. **判準改主按鈕**：probe 加回 `alternate_greetings: usize`（包 4 之後刪過，這次是判準用途）；主按鈕條件＝`book_shaped || lorebook_heavy || alternate_greetings > 0`。
3. **身分框全開**：拿掉「純角色卡零詢問直匯」，一律跳身分框、判準只決定主按鈕與文案。**唯一例外保留**：`!name || book_shaped`（頂層就是 entries 的獨立書，沒有角色可建，「匯入成角色卡」是假選項）維持零打擾直匯。

## Next action
從上面「身分判定改版」第 1 步開工。做完再回頭看：驗收七項只剩 4、7 兩項，且都卡在**沒有合適的樣本卡**（4 要能疊出多筆匯入紀錄的一批卡，7 要一組成對的角色卡＋世界書 JSON）。取得素材後補驗即可結案（狀態改 completed、TASKS.md 行移 DONE.md、本檔 Completed 搬 archive）；在那之前本任務等同做完，不必再排工。
專案下一件事不在本任務：[ai-card-refactor](../tasks/ai-card-refactor.md) 的免費前置——樣本卡逐一匯入、看未收編帳本分佈（零額度），據分佈細拍分包。那件事本來就要一批樣本卡，屆時順手把 4、7 一起驗掉。訓帝卡的診斷結論（雲端載入器卡、介面在作者網站上）已記在上面，那類卡的介面本地化正是該任務拍板 3 的目標。

## Constraints
- AI 卡重構按鈕另案（等 state-values-mvu 真桌實跑的帳本分佈再細拍）；入口方向已拍進任務檔不做清單。
- 新 UI 字串十語系逐鍵（zh-TW 正典）；驗證四件套：cargo test＋npm run build＋npm test＋check:i18n。
