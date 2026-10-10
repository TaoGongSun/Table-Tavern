# worldbook-character-visibility 施工方案

交接見 [handoffs/worldbook-character-visibility.md](../handoffs/worldbook-character-visibility.md)。行號以分支起點 `950a771` 為準。

## 一、現況

### 可見度的寫入與讀取
- 條目可見度存在 `extensions.table_tavern.visibility`，三值 `Gm`／`Public`／`Characters(ids)`（`src-tauri/src/data/worldbook.rs:16`）。讀不到或格式不對一律當 `Gm`（`worldbook.rs:74-95`）。
- 匯入時沒帶這個欄位的條目補上呼叫端給的預設（`normalize_imported_entry`，`worldbook.rs:714-721`）；`import_worldbook` 固定給 `Gm`（`:802`），`import_worldbook_as` 由呼叫端指定（`:817`）。
- 同一桌重複匯入用內容指紋去重（標題、內文、兩組關鍵字，**不看可見度、constant、disabled、order**，`worldbook.rs:757-784`、`:845-863`）：重複的來源條目直接映到桌上已有那條，可見度不動。

### 四條匯入路
| 路 | 位置 | 卡內條目沒指定可見度時 |
|---|---|---|
| 角色卡路（桌面版） | `import/card.rs:306` → `import_character_placing`（`:322`），`BookVisibility::Gm`（`:373-376`），失敗只略過 | `Gm` |
| 網頁存檔·角色卡路（D16） | `import/web_save/mod.rs:177-184`，`BookVisibility::OwnCharacter`（`card.rs:377-386`），失敗整次回錯 | `Characters([新角色 id])` |
| 世界書路（桌面版） | `import/files.rs:58-69` → `worldbook_json`（`card.rs:549`）剝出 `character_book`／獨立 V2 書／人設欄合成條目 → `data::import_worldbook` | `Gm` |
| 網頁存檔·世界書路 | `web_save/mod.rs:191-194` | `Gm` |
| AI 重構套用 | `refactor/apply.rs:264-283`：照搬型（carry）條目原樣保留來源可見度；AI 重寫／本地合組的新條目、沒勾成卡的人物條目一律 `Gm`（`:222`、`:281`） | 不是匯入，但會吸收並刪除來源條目（`:391`），見二之 7 |

`BookVisibility` 這個列舉同時代表兩件事：可見度預設與「隨身書寫不進去要不要整次回錯」（`card.rs:356` `strict`）。

### 消費端
- GM 線：凍結 system 收**全部** constant 條目、不分可見度（`transport/turns.rs:264-283` `gm_lane_system`；單發 `transport/assemble.rs:107-151` `assemble_gm_messages`）；keyword 條目全收進回合尾段。GM 另外看得到所有卡的私設全文。
- 角色線：凍結 system 只收 `Public` constant（`turns.rs:117-122`）；回合尾段先濾可見度（`turns.rs:160-167`：`Gm` 濾掉、`Characters` 只給名單內的角色），`Characters` 條目不論 constant 都走機密段「只有「X」知道的世界情報」逐輪注入、回合後抹掉（`turns.rs:171-179`、`:216-227`）。API 共線（`transport/assemble.rs:44-49`）同一套。
- 角色側觸發用 `active_worldbook_entries`（`context.rs:8-32`）對角色看得到的事件比對。

### 關鍵發現：卡內世界書其實已經整包塞進私設
`import/card.rs:496-520` `private_markdown` 把 `character_book` 每一條（只濾空內文，**不看 enabled、constant、keys**）寫成 `- **關鍵字**：內容` 放進卡的 `private_md`；角色線把 `private_md` 整段放進機密段（`turns.rs:195-214`）。所以：
- 角色現在「看得到」自己卡的內容，但是以永遠常駐的私設文字呈現：keyword 條目不按觸發、停用條目照送、`[initvar]`、EJS 腳本、`[mvu_update]` 規則表與輸出格式條目也全文送進角色線。
- 立案時說的「角色看不到」準確說法是：**條目層的 ST 行為（觸發、啟停、順序）角色線一條都沒有，取而代之的是一份不分啟停的全文傾印**。
- 網頁存檔路（D16）同時有傾印與 `Characters` 條目：constant 條目在角色線出現兩次。
- 匯出（`import/export.rs:58`、`:140-185` `character_book`）是從 `private_md` 的這種行反向解析回 `character_book`，不讀世界書；其餘私有筆記併成一條常駐條目（`export/tests.rs:108` `exports_freeform_card_as_json`）。備用開場白：匯入時寫成私設裡的 `### 備用開場白 n` 段（`card.rs:527-543`），匯出卻固定寫 `alternate_greetings: []`（`export.rs:47`），開場白段被當成私有筆記併進常駐條目。
- 收據：撤銷時「改寫過的既有條目」整條覆寫回原快照（`receipts.rs:846-853`），會抹掉匯入後玩家改的內容；收據指紋含可見度（`receipts.rs:455-471`、撤銷比對 `:828-835`）。網頁存檔匯入一律建新桌、不留收據。
- 書匯入失敗被吞：桌面版角色卡路 `card.rs:375` `.ok()`；單條 `enabled` 不是布林就整本失敗（`worldbook.rs:706`）。

因此只改預設可見度不夠（會變成雙份）；本案要同時處理傾印。

### 前端
世界書編輯器已支援三種可見度與角色勾選（`src/features/worldbook/WorldbookEntryForm.tsx:94-120`）；但 `useWorldbookEditor.ts:106-114` `persistDraft` 存檔時會把名單裡不在目前角色清單上的 id（例如已封存的角色）悄悄濾掉。

## 二、做法

觸發沿用桌面版既有子集（constant＋最近 4 則事件的主鍵子字串比對，`context.rs:8-32`），本案不動；完整對齊 SillyTavern 由 worldbook-st-trigger-parity 另案處理。

### 1. 預設值：「這張卡由誰演，條目就給誰看」，在匯入的呼叫端決定
- 角色卡路（桌面版與網頁存檔一致）：沒指定可見度的條目設 `Characters([新角色 id])`；明示的 `gm`／`public` 原樣保留。明示的 `characters` 名單裡的 id 全都不在本桌、或可見度值讀不懂，一律當成沒寫、套預設——順便兜住舊版匯出、帶著別桌 id 的卡〔主線裁決 2026-10-10，第 1 輪審查〕。
- 世界書路（桌面版與網頁存檔一致）：維持 `Gm`。世界書路的卡由 GM 演，對等規則是「給演這張卡的人」。單獨匯入的 V2 世界書 JSON（不是卡）也維持只給 GM〔作者裁決 2026-10-10〕。
- 讀取端（`visibility_from_value`）讀不到或格式壞掉仍退回 `Gm`（三方審查同意保留）；「缺欄位」與「格式壞掉」分開測。
- `BookVisibility` 拆掉：可見度一律走上面的規則，原本混在裡面的「嚴格／盡力」改成獨立參數（桌面版盡力、網頁存檔嚴格）。兩條世界書路共用 `import_worldbook_as(…, &Visibility::Gm)`，對等規則寫成一句註解放在角色卡路那個分支。
- 角色卡路匯入的每條條目另記來源卡：`extensions.table_tavern.source_cards: [卡 id]`（去重命中時取聯集）。匯出靠它收明寫 `gm`／`public` 的條目，見二之 3。

### 2. 拿掉 `private_md` 的條目傾印〔作者裁決 2026-10-10〕
- `private_markdown` 不再寫 `character_book` 條目，只留備用開場白段。角色線改由 `Characters` 條目照觸發拿內容；GM 線照舊看得到全部條目（constant 進凍結 system、keyword 命中進尾段）。
- 卡裡的輸出格式、狀態規則條目不對角色藏，照一般條目給該角色看（照 ST）〔作者裁決 2026-10-10〕。角色台詞目前不剝狀態欄（`lanes/mod.rs:936-940`），角色照抄格式時會原樣顯示，列在範圍外發現。
- 提進 system〔主線裁決 2026-10-10，第 2 輪審查〕：在 `chars_lane_turn` 裡，`hoist_private` 為真時，本卡看得到的 constant `Characters` 條目跟私設走同一條 hoist 路（併進 `hoisted_private` 那塊、從尾段機密段拿掉）。三個消費端因此一起生效：單卡 API 桌（`assemble.rs:55-58`）、單人在場的 Claude 與 Agy 線（`chat_assembly.rs:225-237`，形狀見 `lanes/mod.rs:184-192`）。Agy 回合後不抹尾段（`:186`），constant 條目若留在尾段每輪會疊一份，所以一定要提走。keyword 命中的 `Characters` 條目照舊在尾段（與 `Public` keyword 條目現行行為相同）。不 hoist 的線照舊走尾段機密段、回合後抹掉。

### 3. 匯出改讀世界書原始條目〔主線裁決 2026-10-10，第 1 輪審查〕
- 世界書讀取或解析失敗＝整次匯出失敗，不產出檔案；沒有世界書檔＝沒有條目，不算錯〔主線裁決 2026-10-10，施工驗收第 1 輪〕。
- `character_card_v2` 讀這桌世界書的原始條目 JSON（`read_worldbook_value`），收「`Characters` 名單含此卡 id」或「`source_cards` 含此卡 id」的條目，轉回 V2 `character_book`：`key`→`keys`、`keysecondary`→`secondary_keys`、`order`→`insertion_order`、`disable`→`enabled`，`selective`、其餘 `extensions` 原樣帶，拿掉 `uid`、`displayIndex`。不從精簡的 `WorldbookEntry` 重建。
- V2 格式正規化〔主線裁決 2026-10-10，第 3 輪審查〕：照 ST `convertWorldInfoToCharacterBook` 的寫法——`position` 已經是字串的原樣保留；數字 `0`→`before_char`，其他非零值→`after_char`，原數字留在 `extensions.position`（ST 匯入時優先讀它）。`case_sensitive`：原始條目已有 snake_case 的 `case_sensitive`（角色卡匯入保留下來的）照原值；沒有才從 `caseSensitive` 轉（`null`→`false`）。編輯器建的條目是數字 `0` 與 `caseSensitive`，都會轉成 V2 寫法。
- 可見度怎麼寫出去：名單是 `Characters`（不論是否還含別的角色）→ 不寫 `visibility`，讓下一桌重新套預設；明寫 `gm`／`public` 的才寫出去。
- `source_cards` 從所有匯出條目上拿掉，匯入時在新桌重建歸屬〔主線裁決 2026-10-10，第 2 輪審查〕。
- 鷹架停用還原〔主線裁決 2026-10-10，第 4 輪審查〕：匯入時被判成機制鷹架而強制停用的條目（`worldbook.rs:722-726`），寫兩個標記：`extensions.table_tavern.source_disable`（來源原本的停用值，永久保留、進去重指紋）與 `extensions.table_tavern.forced_disable: true`（「目前的停用是匯入時強制的」）。`update_entry_fields`（`upsert_worldbook_entry` 的既有條目路徑；帳本開關 `ledger.rs:196` 與編輯器存檔都走這裡）只在 `disable` 值有變時清掉 `forced_disable`。匯出時只有 `forced_disable` 還在才把 `enabled` 還原成 `source_disable`，否則照玩家現在的值；兩個標記匯出時都拿掉。玩家啟用後又停用，匯出就是停用；帳本開關翻過也不影響同卡再匯入的去重。來源條目已經帶 `source_disable`（這桌匯出的世界書再匯回來）就沿用，不從目前的 disable 推〔主線裁決 2026-10-10，施工驗收第 1 輪〕。
- 已知限制：沒有 `source_cards` 的條目（世界書路匯入、玩家手建）即使可見度是 `gm`／`public` 也不會跟著角色卡匯出；要帶走就在編輯器把它設成該角色可見。
- 私有筆記反向解析：只刪掉把 `- **關鍵字**：內容` 行轉成條目那段；其餘私有筆記（含玩家手寫的這種行）照現行併成一條常駐條目匯出（`exports_freeform_card_as_json` 續綠）。
- 備用開場白〔主線裁決 2026-10-10，第 2 輪審查〕：反解回 `alternate_greetings`，`export.rs:47` 的空陣列改掉。規則：
  - 段頭＝圍欄外、整行（去掉行尾空白）等於十語系 `ALTERNATE_GREETING` 段標（`card.rs:103`，`{n}` 為正整數）的行；判圍欄沿用 `export.rs:108-137` 既有的 CommonMark 圍欄判定，圍欄裡長得像段標的行不算。
  - 一段開場白從段頭下一行到「下一個段頭」或私設結尾為止；開場白內文自帶的其他標題（例如 `### 第一章`）留在該段。
  - 第一個段頭之前的內容是私有筆記。最後一段之後的內容無法與開場白區分，一律算進最後一段開場白——已知限制：玩家把私有筆記寫在開場白段後面，匯出時會併進最後一則開場白；匯入寫的順序是開場白段在最後，玩家要另加筆記請寫在開場白段之前。

### 4. 去重：合併可見度，不吞掉別的角色的份〔主線裁決 2026-10-10，第 1 輪審查〕
- 指紋〔主線裁決 2026-10-10，第 2 輪審查〕：標題、內文、主鍵、次要鍵、正規化後的 `constant`（缺欄＝`false`）、`source_disable`（只有鷹架條目才有；其他條目缺欄）。**不含** `disabled`、`order`：玩家用帳本開關或編輯器改過這兩欄後，同一張卡再匯入不會多出重複條目。含 `source_disable`：鷹架條目正規化後都變成停用，原本一啟一停的兩條靠它分開，匯出時各自還原。
- 指紋命中時，依「這條來源條目正規化後的實際可見度」（不看呼叫端的預設值）與桌上那條合併：`Public` 優先 → `Characters` 取聯集 → `Gm` 最低。
- 兩種 `Gm` 分開〔主線裁決 2026-10-10，第 2 輪審查〕：
  - 落定的明示 `Gm`＝桌上那條是 `Gm` 且帶 `source_cards`（角色卡路來源明寫 `gm`，或玩家在編輯器把角色卡條目改成 GM）：被沒寫可見度的同指紋條目命中時維持 `Gm`，只把新卡併進 `source_cards`；只有來源明寫 `public` 能把它升成 `Public`。
  - 預設 `Gm`＝`Gm` 且沒有 `source_cards`（世界書路匯入、玩家手建）：照一般規則，可被 `Characters` 合併成 `Characters([卡 id])`。
  - 來源明寫 `gm` 的條目命中桌上任何條目：結果就是桌上原值，不擴大。
- 合併寫成一個 helper（例如 `merge_entry_visibility`，同時處理可見度與 `source_cards` 聯集），`import_worldbook_as` 與世界書編輯器的「清重複」`dedupe_worldbook`（`worldbook.rs:899-918`）共用：清重複保留顯示順序最前那條時，把被刪那條併進來。
- 情境：同一張卡在同一桌匯兩次 → 兩張卡都看得到；先走世界書路（預設 `Gm`）再把同卡當角色卡匯入 → 變成 `Characters([卡 id])`。

### 5. 收據與撤銷〔主線裁決 2026-10-10，第 1 輪審查〕
- `BookImport` 多帶「被合併改寫的既有條目」：uid、改寫前的 `visibility` 與 `source_cards`、改寫後的值。凡是匯入前就在桌上、且可見度或 `source_cards` 任一有變的都記（只多了 `source_cards` 也要記，否則撤銷後還殘留已刪那張卡的歸屬〔主線裁決 2026-10-10，第 2 輪審查〕）。
- 同一批多次命中〔主線裁決 2026-10-10，第 2 輪審查〕：同一 uid 在同一次匯入裡被命中多次，收據記「最初的 before」，after 以這次匯入結束時的實際值為準；本次匯入新建、後來又被同批條目合併的條目不進這份收據（它整條屬於本次新建，撤銷時整條刪）。
- 兩條路都傳〔主線裁決 2026-10-10，第 2 輪審查〕：角色卡路經 `import_character_placing` → `import_character_reporting` → `import_character_file` 傳進 `record_character_import`；世界書路也可能因來源明寫 `public` 而擴權，`import_worldbook_file`（`files.rs:69` 改用帶回 `BookImport` 的版本）傳進 `record_worldbook_import`。收據新增欄位（例如 `visibility_restores`），不併進 `rewritten_entries`。只有改寫、沒有新增的世界書匯入算有效收據，`receipts.rs:574` 的 NothingNew 判斷要把它算進去。網頁存檔匯入一律建新桌、不留收據，不需要。
- 撤銷：這類改寫只還原 `visibility` 與 `source_cards` 兩個欄位，不整條覆寫，玩家匯入後改的內容保留。目前值不等於收據記的 after（玩家自己改過可見度，或按過清重複）就不動、計入保留數。
- 指紋含可見度（`receipts.rs:455-471`）的影響：撤銷只能撤最後一筆；後一筆匯入擴大了前一筆收據的條目時，先撤後一筆會把可見度還原，前一筆的指紋就對得上。撤銷程序先做可見度還原、再比對本收據新建條目的指紋。這個論證只在中間沒有玩家改動、沒按清重複時成立〔主線裁決 2026-10-10，第 2 輪審查〕：玩家改過可見度時跳過還原，前一筆收據撤銷時那條指紋對不上、被當成玩家改過而保留，這是合理結果（玩家改過的不刪）。

- 重構撤銷保留原始欄位〔主線裁決 2026-10-10，第 3 輪審查〕〔主線裁決 2026-10-10，第 4 輪審查〕：收據的 `deleted_entries`（`receipts.rs:203-207`）現在存精簡 `WorldbookEntry`，撤銷插回（`:869-895`）會掉 `source_cards`、`source_disable`、`keysecondary`、`position`。收據多存一份對應的原始 JSON（新欄位加 `serde(default)`，舊收據缺欄照舊走精簡插回），撤銷時走二之 7 的「從原始值插入」入口：
  - 「已插回」判定（`:860-872`）：新收據改比原始 JSON（解析後的值相等，不比字串，欄位順序不同不算差異〔主線裁決 2026-10-10，第 5 輪審查〕），只排除 uid 與 displayIndex；不然只差次要鍵、`source_cards`、`position` 的條目會被誤判成已插回而跳過。舊收據照現行比精簡欄位。
  - 原 uid 被佔走、改插新 uid 的那條（`:873-879`）也走原始值插入，「全欄位相同已在就跳過」同樣比原始 JSON。
  - 套用中途失敗的收據：原始 JSON 跟著 `failure.progress`（`refactor/apply_record.rs:43`）一起寫進失敗收據，自動回滾插回時同樣保有原始欄位。

### 6. 書匯入失敗要讓玩家知道〔主線裁決 2026-10-10，第 1 輪審查〕
- 桌面版角色卡路不再 `.ok()` 吞掉：角色本體照舊建立，但匯入結果多帶一個「隨身世界書沒匯成」旗標（比照 `image_dropped`，`commands/character.rs:106`），前端提示一行。
- 單條 `enabled` 不是布林（`worldbook.rs:706`）：該條略過、其餘照匯；網頁存檔嚴格模式照舊整次回錯。
- 略過條數與重複條數分開回報〔主線裁決 2026-10-10，第 2 輪審查〕：`WorldbookImport`（`worldbook.rs:792`）加一欄（例如 `invalid`）。`commands/character.rs:97-104` 現在用「探測條數－新增數」當重複數，改成直接用 `BookImport` 回來的 `skipped` 與 `invalid`。

### 7. AI 重構：新條目沿用來源角色的可見度與觸發〔主線裁決 2026-10-10，第 1 輪審查〕
原方案寫「重構不改」，第 1 輪審查推翻。問題：`refactor/apply.rs:276-282` 沒帶 meta 的新條目一律 `keys=[]`、`constant=false`、`Gm`，`:391` 又刪掉被吸收的來源條目。GM 不是整本讀：`active_worldbook_entries`（`context.rs:20-28`）只收 constant 或主鍵命中的，`gm_lane_system` 只收 constant，尾段只收命中的 keyword（`assemble.rs:112-114`、`turns.rs:301`）。這些新條目過去靠私設傾印才有人看得到；傾印拿掉後角色和 GM 都看不到，是本案造成的退步，所以一併處理〔主線裁決 2026-10-10，第 3 輪審查〕。
- 來源一律讀套用前的快照〔主線裁決 2026-10-10，第 2 輪審查〕：`apply.rs:164` 的 `existing_entries`，另在同一處讀一份原始條目 JSON 快照（給 ST 欄位與 `source_cards` 用），因為 `:391` 會刪掉來源條目。
- 來源身分核對〔主線裁決 2026-10-10，第 3 輪審查〕〔主線裁決 2026-10-10，第 4 輪審查〕：重構卡可以跨桌套用（`refactor/card_file.rs:32` 只帶 outcome、meta、來源 UID），新桌可能找不到來源，或同一 UID 指向無關的條目。outcome 新增 `source_fingerprints: uid→身分指紋`（`serde(default)`），涵蓋 outcome 引用到的**所有** uid（characters、entries、mechanisms、interface 的 `source_uids`／`source_uid`，以及 `dropped`、`preserve_source_uids`）：實作上由 `refactor_survey` 在送 AI 之前取整桌條目的指紋（`RefactorSurveyOutcome.source_fingerprints`），前端照抄進產物（`assembleRefactorOutcome`），重構卡匯出匯入一路帶著。
  - 身分指紋的範圍：標題、內文、主鍵、次要鍵、正規化後的 `constant`。不含 `disabled`、`order`、可見度、`source_cards`、`source_disable`、`forced_disable`、`is_person`、`locked`、uid、displayIndex——玩家在 survey 與套用之間只動開關、順序或可見度，來源仍認得出來。與二之 4 的去重指紋是兩個函式（去重指紋另含 `source_disable`）。
  - 套用開頭一次算出「已核對來源表」：outcome 裡的 uid 先照 UID 在快照裡找、身分指紋相符才算；不符或找不到，再用身分指紋在快照裡反查，**恰好一條**相符就改對到那條的 uid（同一張原卡匯到新桌、UID 不同的情境），零條或多條都算核對失敗。outcome 沒有 `source_fingerprints`（舊產物、既有測試手組的產物）時只照 UID 對，與現行相同。
  - 一對一〔主線裁決 2026-10-10，第 5 輪審查〕：同一條實際條目被多個 outcome uid 對到時，這些 uid 全部算核對失敗。
  - 映射後一律用實際 uid〔主線裁決 2026-10-10，第 5 輪審查〕：`preserve_source_uids`（`apply.rs:380`）換成實際 uid 再擋刪除；`uid_owners`、`source_consumers` 用映射後的 uid 聚合。
  - 核對失敗的 uid 一律視同不在 `preexisting_uids`：不刪（`apply.rs:385-391`）、不停用（dropped，`:404-425`）、不掛帳本紀錄（absorbed_ledger_record）、機制與介面的 `source_uid` 不停用，也不算進 `uid_owners`、`source_consumers`。套用裡所有讀來源 uid 的地方都改查這張表，不各自判斷。
  - 退回：carry 條目來源核對失敗 → 照現行用 meta 建條目（`new_entry_value`），meta 的 `Characters` 名單 id 全不在本桌時給 `Gm`；沒帶 meta 的新條目所有來源都核對失敗 → `Gm`、`constant=true`（內容至少讓 GM 看得到）、`keys=[]`、`source_cards` 空。部分來源核對成功 → 只用成功的那些算可見度與觸發。
  - 同一桌在 survey 之後、套用之前玩家改了來源內容：身分指紋不符 → 來源不刪、新條目照建，新舊並存。這是安全的方向：玩家改過的內容不會被重構吃掉，多出來的條目玩家自己刪。
- 可見度：沒帶 meta 的新條目——所有核對成功的來源的可見度都是同一份 `Characters` 名單就沿用；混合、沒有來源時給 `Gm`。沒勾成卡的人物條目（`:222`）維持 `Gm`〔主線裁決 2026-10-10，第 4 輪審查〕：人物條目登場事件只要不是 `Public` 就是 gm_only（`commands/chat.rs:713`），`Characters` 對角色沒有實際效果，少一個分支。
- 觸發政策〔主線裁決 2026-10-10，第 3 輪審查〕：套用到**所有**沒帶 meta 的 setting 新條目，不論可見度：
  - 只算啟用的來源：`keys`＝啟用來源的主鍵聯集（去重、保留出現順序）；`constant`＝任一啟用來源是 constant；`disabled`＝來源全部停用才停用（此時 keys／constant 改用全部來源計算，維持條目被玩家啟用時的行為）。
  - 理由：新條目內容是從這些來源合成的，「任一啟用的來源會被送出時它就送出」最接近原卡在 ST 的行為。代價：只要有一條啟用的 constant 來源，合成後整條常駐，該條線字數可能比原本多。次要鍵不併（合成後無法保留原本的 AND／NOT 邏輯），列已知限制。
  - 排除：機制條目（`kind=="mechanism"` 且有 rules 或 triggers，即 `apply.rs:262-263` 判成 locked 的）優先於 meta——重寫路徑會產出帶 meta 的機制條目〔主線裁決 2026-10-10，施工驗收第 1 輪〕——不套觸發政策、也不沿用可見度，維持 `keys=[]`、非常駐、`Gm`。沒勾成卡的人物條目（`:222`，`is_person`）可見度與觸發都維持現行——人物條目走登場機制（`transport/arrivals.rs`）。
- 歸屬與 ST 欄位〔主線裁決 2026-10-10，第 2 輪審查〕：
  - carry 條目（來源通過核對時）：從原始快照整條複製來源 JSON（`keysecondary`、`position`、`selective`、`case_sensitive`、`source_cards`、`source_disable` 等全帶），只換 uid、displayIndex、內文；`locked`、`is_person` 用套用端的判定覆寫，不沿用原始值裡的舊旗標〔主線裁決 2026-10-10，第 3 輪審查〕；可見度取已對上的來源條目**目前**的可見度，不用 `meta.visibility`〔主線裁決 2026-10-10，第 5 輪審查〕——A 桌名單 [A角色] 的 carry 套到 B 桌、反查對到 [B角色] 的同卡條目時才不會被覆寫回 A 桌的 id；同桌 survey 後玩家改了可見度也照這條。要在 data 層加一個「從原始值插入」的入口，因為 `new_entry_value`（`worldbook.rs:344`）只寫精簡欄位。`RefactorEntryMeta`（`refactor_ai/types.rs:216`）不必加欄位，carry 在套用端靠 `source_uids` 對回快照。
  - 合組／AI 重寫的新條目：`source_cards`＝存在來源的聯集；`keysecondary`、`position` 用預設值——已知限制：合成條目匯出時不帶次要鍵與非預設位置。
- `refactor_ai/context.rs:138-158` `format_worldbook_entry` 的旗標加可見度（例如「［只給：角色名］」，id 換成角色名），讓 AI 知道這些條目屬於某個既有角色。
- **行為變動**〔主線裁決 2026-10-10，第 3 輪審查〕：世界書路的重構桌也會變——過去合組／重寫的 setting 新條目 keys 空、非常駐，GM 也觸發不到；改後照來源觸發，GM 收到的提示會變多（有 constant 來源的合組條目常駐 GM system）。

### 8. 編輯器保留看不到的 id〔主線裁決 2026-10-10，第 1 輪審查〕
`useWorldbookEditor.ts:106-114` `persistDraft`：名單裡原本就有、但不在目前角色清單上的 id（例如已封存的角色）照留，只濾掉玩家這次取消勾選的。

### 9. 已存桌
不遷移〔作者裁決 2026-10-10〕。發佈前已匯入的桌，角色仍靠私設傾印看條目；匯出改讀世界書後，這些卡匯出時沒有 `character_book`，要正確行為就重新匯入。

## 三、測試（cargo＋vitest，照既有模組放）
計數規則：「恰好一次」「不出現」一律對整份 messages 串起來（system＋歷史＋尾段）計數，不從段標往後切；測試條目文字避開逐字稿出現過的字；觸發關鍵字放在最近 4 則事件之內。
1. 角色卡路·下一輪 messages：卡帶 constant 條目、未命中 keyword 條目、命中 keyword 條目、停用條目、明示 `gm` 條目、constant 的輸出格式條目（`[mvu_update]` 開頭，非規則表）。桌面版 `import_character` 後：
   - 角色線：多卡 lane 訊息（尾段機密段）、API 共線 `assemble_shared_messages`（多卡與單卡）、單人在場 Claude 線、Agy 線，四種都測：constant、命中、格式條目各恰好一次；未命中、停用、明示 GM 不出現。hoist 的三種另斷言 constant 條目在 system、不在尾段。Agy 連跑兩輪〔主線裁決 2026-10-10，第 3 輪審查〕：Agy 續聊時 stdin 只有本輪 prompt、system 在第一輪，所以把第一輪送出的 body 與第二輪 prompt 串起來計數，constant 條目恰好一次；keyword 命中的 `Characters` 條目照舊（與 `Public` keyword 條目相同）每次命中的回合都在該輪 prompt 出現一次，兩輪都命中時串起來是兩次，用測試鎖住。
   - GM 線（`assemble_gm_messages` 與 `gm_lane_system`＋`gm_lane_turn`）：constant 恰好一次、命中與明示 GM 條目出現、未命中與停用不出現；私設不含條目文字。
2. 世界書路對等：同一張卡走 `import_worldbook_file`，條目全為 `Gm`、另加一張無書角色卡的角色線看不到、GM 線看得到；網頁存檔 `worldbook_route_keeps_gm_visibility_and_has_no_character`（`web_save/tests.rs:205`）續綠。
3. 可見度值：缺欄位→角色卡路套預設、讀取端為 `Gm`；格式壞掉→同上，兩者分開；`characters` 名單 id 全不在本桌→套預設。
4. 去重：
   - 同卡匯兩次→兩張卡的角色線都拿得到；先世界書路再角色卡路→`Characters([卡 id])`；來源明寫 `gm` 的重複條目不擴大。
   - 落定的明示 `Gm`（角色卡路來源明寫 `gm`）被同指紋、沒寫可見度的條目命中→仍是 `Gm`、`source_cards` 多一張；世界書路的預設 `Gm` 被命中→變 `Characters`。
   - 同文字不同 constant 兩條各自保留；玩家改過 `disabled`／`order` 後再匯同卡→不多出條目。
   - 鷹架條目一啟一停兩條→各自保留，匯出各自還原；玩家用帳本開關啟用過的鷹架條目匯出照現值；啟用後又停用→`forced_disable` 已清、匯出為停用；鷹架條目被帳本開關翻過後再匯同一張卡→不多出條目。
   - `dedupe_worldbook` 清重複後被刪那條的角色名單與 `source_cards` 併進保留那條。
5. 收據：
   - `import_character_file` 擴大既有條目 → 編輯該條內容 → `undo_last_import`：內容保留、可見度與 `source_cards` 還原。
   - 只多了 `source_cards` 的既有條目撤銷後歸屬還原。
   - 同一批多次命中同一 uid 後撤銷→回到最初值；同批先新建後被合併的條目撤銷時整條刪、不進可見度收據。
   - 玩家改過可見度→撤銷不還原、計入保留；接著撤前一筆→那條被當玩家改過而保留。
   - 前後兩筆匯入依序撤銷，前一筆新建條目照常刪除。
   - 世界書路只有改寫（來源明寫 `public` 合併進既有條目）、沒有新增→留下有效收據，撤銷後可見度還原。
6. 匯出往返：A 桌匯入卡→匯出→B 桌（新桌）匯入→B 的新角色線看得到條目；V2 `character_book` 的 `secondary_keys`、`position`（字串）、`case_sensitive`、`selective`、`enabled` 與原卡一致；編輯器新建的條目（數字 `0`、`caseSensitive`）匯出成 `before_char`、`case_sensitive`；數字非零位置匯出成 `after_char` 且 `extensions.position` 保留原數字；原卡 `case_sensitive: true` 往返後仍是 `true`；明寫 `gm` 條目帶 `visibility: gm` 出去；`Characters` 條目不帶 visibility；匯出條目都沒有 `source_cards`、`source_disable`；`exports_freeform_card_as_json` 續綠。
7. 備用開場白：一般往返回到 `alternate_greetings`；三個反例——開場白內含 `###` 標題（留在該段）、圍欄裡有假段標（不切段）、最後一段後面接私有筆記（併進最後一段，鎖住已知限制）。
8. MVU 卡匯出再匯入：規則、初始樹、incremental 都不變；規則條目匯出時 `enabled` 回到原卡值。
9. 書匯入失敗：角色卡路書壞掉時角色照建、結果帶旗標；單條 `enabled` 非布林時其餘照匯，`invalid` 與 `skipped` 分開回報。
10. AI 重構（都驗實際送出的 messages，不只驗欄位）：
    - 角色卡路匯入 → 套用把該卡條目合組成新條目（無 meta）的產物 → 角色線含新條目內容（有啟用的 constant 來源時無觸發也在；全 keyword 來源時命中來源主鍵才在）、來源條目已刪。
    - 混合來源的新條目給 `Gm`：角色線看不到；GM 線（`assemble_gm_messages`）照觸發政策看得到。世界書路重構桌的合組條目同樣在 GM 線出現。
    - 反例：停用的 constant 來源＋啟用的 keyword 來源合組 → 不常駐，只在命中 keyword 時出現；來源全停用 → 新條目停用。
    - 負向：來源是 `Characters` 常駐條目時，locked 機制條目不進角色線、仍是 `Gm`、非常駐。carry 條目來源原始值帶舊的 `locked`／`is_person` 旗標時，以套用端判定為準。
    - 跨桌：outcome 帶 `source_fingerprints`，套到另一桌——缺來源時 carry 退回 meta、無 meta 條目退回 `Gm`＋常駐；沒有 `source_fingerprints` 的產物照 UID 對。
    - 同 UID、指紋不符的目標條目：套用前後原樣保留（不刪、不停用、沒有帳本紀錄、不被機制或介面 source_uid 停用），且不複製它的原始 JSON。
    - UID 不同、內容相同（同卡匯到新桌）：反查恰好一條時對上並照常吸收刪除；反查到多條時當核對失敗、全部原樣保留。
    - 同桌 survey 後玩家改了來源內容再套用：來源保留、新條目照建；只改開關或順序時照常吸收。
    - carry 跨桌反查對到 B 桌同卡條目：可見度是 B 角色名單，B 角色線實際送出的 messages 看得到該條。
    - 兩個 outcome uid 映到同一條實際條目→兩者都核對失敗、條目保留；兩個來源 uid 映到同一目標、部分勾選的反例（聚合用映射後 uid）。
    - 重構後匯出：carry 條目匯出時 `secondary_keys`、`position`、可見度規則與重構前一致，合組條目照已知限制。
    - 重構撤銷：角色卡路匯入 → 重構 → 撤銷 → 匯出：插回的來源條目保有 `source_cards`、`source_disable`、`keysecondary`、`position`，匯出與重構前相同；舊格式收據（缺原始 JSON 欄）撤銷照舊可插回。
    - 撤銷衝突與重試：原 uid 被佔走時以新 uid 原始值插回；撤銷做到一半失敗再撤一次，不插出第二份；只差次要鍵或 `source_cards` 的條目不被誤判成已插回。
    - 刪來源後套用失敗自動回滾：插回的來源條目保有原始欄位。用測試專用注入點（`refactor/apply.rs` 的 `fail_point`，刪完來源後失敗一次、不寫壞檔）走 `apply_and_record` 的 Err 分支〔主線裁決 2026-10-10，施工驗收第 1 輪〕。
11. 前端 vitest：`persistDraft` 保留不在清單上的 id。
12. 網頁存檔既有 `own_card_entries_become_character_visible_unless_the_card_says_otherwise`（`:157`）、`next_character_turn_carries_constant_and_triggered_entries`（`:173`）、`duplicate_entries_map_to_the_kept_uid`（`:801`）續綠，後兩支改成整份 messages 計數「恰好一次」。
13. 既有斷言匯入後 `Visibility::Gm`、私設含條目文字或舊指紋的測試照新規格改。
14. `npm run verify` 全綠。

## 四、已拍板
- **P1**：拿掉私設裡的整本條目，改成照觸發送；匯出改從世界書讀回 `character_book`〔作者裁決 2026-10-10〕。見二之 2、3。
- **P2**：單獨匯入的世界書 JSON 維持只給 GM〔作者裁決 2026-10-10〕。見二之 1。
- **P3**：卡裡的輸出格式、狀態規則條目不對角色藏〔作者裁決 2026-10-10〕。見二之 2。
- **P4**：舊桌不遷移〔作者裁決 2026-10-10〕。見二之 9。
- 第 1 輪審查（Sol、Grok、Opus 審查）必改 1–6 與建議 7–12 全數納入（二之 1–8、第三節）〔主線裁決 2026-10-10，第 1 輪審查〕。
- 第 2 輪複審必改 A–F 與建議 G–M 全數納入（二之 2–7、第三節）〔主線裁決 2026-10-10，第 2 輪審查〕。
- 第 3 輪複審必改 1–5 與建議 6–9 全數納入（二之 3、5、7，第三節）；原列範圍外的「重構 Gm 新條目 GM 觸發不到」改由本案處理〔主線裁決 2026-10-10，第 3 輪審查〕。
- 第 5 輪：carry 可見度取來源目前值、一對一、映射後 uid 聚合、插回比解析後的值〔主線裁決 2026-10-10，第 5 輪審查〕。
- 第 4 輪複審必改 1–3 與建議 4–7 全數納入（二之 3、5、7，第三節）；沒勾成卡的人物條目維持 `Gm`〔主線裁決 2026-10-10，第 4 輪審查〕。

## 五、分包
一包：data／import 預設、來源卡標記與去重合併、收據還原、私設傾印與 hoist、匯出改寫（含開場白）、書匯入失敗回報、AI 重構可見度、觸發、來源核對與撤銷原始欄位、編輯器保留 id、測試。Opus 施工（匯入、收據還原、去重合併都是資料安全細節）。

## 六、範圍外發現
- `Characters(ids)` 指向的角色被刪除後，條目沒人清，實際只剩 GM 看得到；沒有其他地方處理 `Characters` 清單（`grep Characters(` 只有 card.rs 與 worldbook.rs）。
- 角色台詞不經 `extract_state_block`，角色若吐出 `<UpdateVariable>`／狀態標籤會原樣顯示（P3 不藏之後仍會發生，現況已存在）。
