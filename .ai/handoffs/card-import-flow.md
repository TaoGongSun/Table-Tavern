# Handoff: card-import-flow

## Current state
四包全部實作完成、主線逐包複驗綠、逐包 commit。2026-08-05 一輪實機驗收挖出四件並全數修完、逐項通過（身分框文案、腳本提示、開場白按鈕位置、兩路徑對等化），雙世界書封鎖也過。剩 Remaining 清單其餘五項未驗。

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

## Verification
- 最終四件套（2026-08-05 主線複驗）：cargo test 327 綠、vitest 22 綠（含 import-routing 5 例）、npm run build ✓、check:i18n 十語系 82 鈕 OK。
- 文案改寫自驗（2026-08-05）：npm run build ✓、vitest 22 綠、check:i18n 九語系 OK（按鈕加長後仍在寬度上限內）；接線見 [App.tsx:5546](../../src/App.tsx:5546)。
- 刪腳本提示自驗（2026-08-05）：cargo test 326 綠（原 327，刪掉兩個只驗 scripts 欄位的測試、新增 probe_ignores_invalid_bytes 保留格式錯誤那條斷言）、npm run build ✓、vitest 22 綠、check:i18n 九語系 OK；殘留檢查 grep importScriptNotice／worldbookScriptNotice = 0。淨變化 −75 行。
- 開場白按鈕改位自驗＋使用者實機確認通過（2026-08-05）：npm run build ✓、vitest 22 綠、check:i18n 九語系 OK。
- 世界書路徑說明自驗（2026-08-05）：npm run build ✓、vitest 22 綠、check:i18n 九語系 OK。原始卡檔實地拆解見 history 當日紀錄。
- 路徑對等化：使用者實機測試通過（2026-08-05）。自驗：cargo test 326 綠（既有 round-trip 測試延伸驗「同一份匯出檔改用世界書身分匯入，機制照樣收進來」）、npm run build ✓、vitest 22 綠、check:i18n 九語系 OK。
- 實機驗收第 6 項通過（2026-08-05 使用者）：新開的桌匯過世界書後再匯世界書會被擋。舊桌（收據功能之前建立的）沒有 `import-receipts.json` 擋不住——使用者拍板**不補**，新功能不覆蓋舊桌可接受。
- 逐包 commit：548905a（包 1）、1939af6（包 2）、d8abe8b（包 3）、包 4 見本次 commit。
- 主線抽讀確認：decideImportRoute 決策表逐條符合拍板；路由框 block 版 JSX 只有取消＋開新桌（App.tsx 搜 -a "importRoute !== null"）；openNewTableAndImport 的 worldId 全程來自 create_world 回傳值。

## Remaining
使用者實機驗收清單：
1. 匯帶書的角色卡 → 三鍵框（份量重的一邊是主按鈕）→ 選角色卡：角色上桌、附帶條目進世界書、自動名桌桌名變卡名。
2. 純世界書 JSON 丟側欄「匯入卡」→ 不問直接進世界書、對話目標指到 GM、桌名用書名／檔名。
3. GM 編輯世界書區只剩「新增條目／去重／匯出」。
4. 「復原上次匯入」按一下退整筆（角色＋條目＋桌名一起回去），可連按逐筆退；改過的條目保留並提示保留數。
5. 已有卡的桌再匯卡 → 路由框；選「開新桌並匯入」→ 新桌名＝卡名、原桌完全不動。
6. ~~已匯過世界書的桌再匯世界書 → 只有「開新桌並匯入／取消」~~（2026-08-05 通過，限新桌）。
7. 有角色卡的桌補匯配套世界書 JSON → 不跳框直接進。

## Next action
等使用者實機驗收 Remaining 剩下的五項（1、2、3、4、5、7 扣掉已過的 6）；過了就結案（狀態改 completed、TASKS.md 行移 DONE.md、本檔 Completed 搬 archive）。驗收若再挖出問題，修完再走一輪四件套。
專案下一件事不在本任務：[ai-card-refactor](../tasks/ai-card-refactor.md) 的免費前置——樣本卡逐一匯入、看未收編帳本分佈（零額度），據分佈細拍分包。訓帝卡的診斷結論（雲端載入器卡、介面在作者網站上）已記在上面，那類卡的介面本地化正是該任務拍板 3 的目標。

## Constraints
- AI 卡重構按鈕另案（等 state-values-mvu 真桌實跑的帳本分佈再細拍）；入口方向已拍進任務檔不做清單。
- 新 UI 字串十語系逐鍵（zh-TW 正典）；驗證四件套：cargo test＋npm run build＋npm test＋check:i18n。
