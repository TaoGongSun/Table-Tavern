# Completed: card-import-flow

（自交接檔搬出的已完成項目與驗證證據，全部已通過使用者實機驗收；現場狀態見 [../card-import-flow.md](../card-import-flow.md)）

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

- **身分判定改版**（2026-08-06 三步全做完，推翻拍板 1 的「純角色卡零詢問直匯」）：
  1. `worldbook_json` 改成三段判斷——`character_book` 條目非空就剝那層（原行為）／頂層有 `entries` 原樣通過（V2 獨立書）／兩者皆無時走新的 `persona_as_worldbook()`，把非空人設欄併成一條沒關鍵字的常駐條目（`comment` 用卡名當標題）。人設欄清單抽成 `PERSONA_FIELDS`，`lorebook_heavy` 秤重改用同一份（原本內嵌重複一次）。**刻意不含 `first_mes`**——開場白走 card_openings 讓玩家挑，收進條目會每回合重複注入。**有條目的卡完全不動**（人設欄要不要一併收另案，避免回歸既有驗收）。人設欄全空回錯不靜默塞空書。
  2. probe 加回 `alternate_greetings: usize`（包 4 之後刪過，這次是判準用途不是提示）。
  3. 前端分流從三條縮成兩條：`!name || book_shaped` 維持零打擾直匯（沒有角色可建），**其餘一律跳身分框**。判準抽成 `looksLikeWorldbook()`（[App.tsx:73](../../src/App.tsx:73)）＝`book_shaped || lorebook_heavy || alternate_greetings > 0`，只決定主按鈕與文案講哪一種。`importChoice` 狀態由存整包 `probe` 改成只存算好的 `booksFirst`。零新 i18n 字串（兩種文案本來就都在）。

- **收據為空時的條目保險**（2026-08-06 實機踩到）：純世界書檔匯進「有內容但沒有收據」的桌（收據功能之前的舊桌／手建桌／範例桌）不會跳確認框，第二本書無聲合進去（使用者靠復原救回）。`decideImportRoute` 加第四個參數 `tableHasWorldbookEntries`，**只在 `receiptKinds` 為空那一格生效、且只管世界書身分**；收據非空時決策表一字未動。使用者立規：**收據為主，條目最多當保險**——不為測試期間的意外改寫長期判準，等所有桌都有收據後那一格自然不再觸發。前端 `tableHasWorldbookEntries()` 現讀 `read_worldbook`（條目歸 WorldEditor 管，App 沒有同步的一份），讀不到當有——寧可多問一句。

- **角色卡路徑也給開場白**（2026-08-06，兩路徑對等化最後一項單邊落地）：原本 `offerOpeningLine` 只在 `importAsWorldbook` 呼叫，理由是「角色卡的開場白已在卡上、不必 GM 再貼一次」——但實際上**沒有任何地方會貼它**，作者寫的第一句話匯完就只躺在卡的「### 開場白」欄位裡（備用開場白落在私有筆記「### 備用開場白 N」），玩家得自己開卡複製。使用者拍板：**貼出形式維持旁白不改成角色發言**，因為開場白不一定是那個角色說的話，也常是場景或角色本身的描寫。`importAsCharacter` 補呼叫同一個函式（同一面板、同一 `post_opening`，該指令本來就不吃 speaker）。判準維持 `alternate_greetings > 0` 不動（「開場白總數 ≥2」的統一提案作廢）。

## Verification
- 最終四件套（2026-08-05 主線複驗）：cargo test 327 綠、vitest 22 綠（含 import-routing 5 例）、npm run build ✓、check:i18n 十語系 82 鈕 OK。
- 文案改寫自驗（2026-08-05）：npm run build ✓、vitest 22 綠、check:i18n 九語系 OK（按鈕加長後仍在寬度上限內）；接線見 [App.tsx:5546](../../src/App.tsx:5546)。
- 刪腳本提示自驗（2026-08-05）：cargo test 326 綠（原 327，刪掉兩個只驗 scripts 欄位的測試、新增 probe_ignores_invalid_bytes 保留格式錯誤那條斷言）、npm run build ✓、vitest 22 綠、check:i18n 九語系 OK；殘留檢查 grep importScriptNotice／worldbookScriptNotice = 0。淨變化 −75 行。
- 開場白按鈕改位自驗＋使用者實機確認通過（2026-08-05）：npm run build ✓、vitest 22 綠、check:i18n 九語系 OK。
- 世界書路徑說明自驗（2026-08-05）：npm run build ✓、vitest 22 綠、check:i18n 九語系 OK。原始卡檔實地拆解見 history 當日紀錄。
- 路徑對等化：使用者實機測試通過（2026-08-05）。自驗：cargo test 326 綠（既有 round-trip 測試延伸驗「同一份匯出檔改用世界書身分匯入，機制照樣收進來」）、npm run build ✓、vitest 22 綠、check:i18n 九語系 OK。
- 實機驗收第 6 項通過（2026-08-05 使用者）：新開的桌匯過世界書後再匯世界書會被擋。舊桌（收據功能之前建立的）沒有 `import-receipts.json` 擋不住——使用者拍板**不補**，新功能不覆蓋舊桌可接受。
- 雙世界書改可融合自驗（2026-08-06）：npm run build ✓、vitest 22 綠、check:i18n 十語系 81 鈕 OK；殘留檢查 grep `importRouteBlock|block_double_worldbook` = 0；淨變化 +75 −42 行（13 檔）。**使用者實機驗收通過**（2026-08-06）。
- 身分判定改版自驗（2026-08-06）：cargo test **327** 綠（原 326，新增 `worldbook_json_converts_persona_fields_when_card_has_no_entries`，並在既有 probe 測試補情境卡案例）、npm run build ✓、vitest 22 綠、check:i18n 九語系 OK；淨變化 +152 −22 行（2 檔）。
- **真卡端到端**（2026-08-06，臨時測試跑完即刪）：`main_furry-male-scenarios-36317429ed88` → `alt_greetings=29`、`book_entries=0`、匯入 1 條常駐條目 1882 字、標題「Furry male Scenarios」、`card_openings` 30 條；`…-d88115666fe1` → 10／0／1895 字／11 條。1882 = description 1873 ＋ `\n\n` ＋ mes_example 7，開場白確實沒被收進條目。
- 條目保險自驗（2026-08-06）：npm run build ✓、vitest **23** 綠（+1 組保險案例，含「角色卡帶進來的條目不會把配套世界書從 companion 變成要問」這條反向釘樁）、check:i18n 九語系 OK；Rust 未動（cargo 維持 327）。
- 角色卡開場白自驗（2026-08-06）：npm run build ✓、vitest 23 綠、check:i18n 九語系 OK；Rust 未動。淨變化 +4 −3 行（1 檔）。四個 `importAsCharacter` 呼叫點全是匯入路徑，都該給開場白。
- 逐包 commit：548905a（包 1）、1939af6（包 2）、d8abe8b（包 3）、包 4 見 b4acab7、雙世界書融合見 a73ca94。
- 主線抽讀確認：decideImportRoute 決策表逐條符合拍板；路由框 block 版 JSX 只有取消＋開新桌（App.tsx 搜 -a "importRoute !== null"）；openNewTableAndImport 的 worldId 全程來自 create_world 回傳值。
