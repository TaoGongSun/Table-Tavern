# 卡片契約：卡片正規化檢視

網頁版與桌面版讀同一張卡要得到同一個結果（計畫 web-version 二之二）。兩邊各把卡算成下面這份 JSON，對同一份 `golden.json` 比對：桌面版 `src-tauri/src/import/card_view_tests.rs`，網頁版 `web/src/features/cards/card-view.ts`。改契約＝兩邊與黃金檔同一筆 commit 一起改。

## 解析
- PNG：逐 chunk 走（不驗 CRC，chunk 頭不足 12 bytes 或長度越界＝`png_invalid`），只讀 `tEXt`；關鍵字 `chara` 優先、沒有才 `ccv3`，同關鍵字取第一個。`iTXt`／`zTXt` 不讀。值是嚴格 base64（長度 4 的倍數、`=` 只在結尾）→ 失敗 `card_data_invalid`；都沒有＝`card_png_no_data`。
- 非 PNG 一律當 JSON；UTF-8 或 JSON 不合法＝`card_json_invalid`。
- 卡資料：頂層 `data` 是物件就取它（V2／V3），否則頂層本身（V1）。外殼原樣保留。
- 失敗時檢視只有 `{"error": "<code>"}`。

## 檢視欄位
- `source`：`png:chara`／`png:ccv3`／`json`。
- `shell`：`wrapped`（有 `data` 物件）、`spec`、`spec_version`（字串或 null）、`extra_keys`（wrapped 時頂層除 spec／spec_version／data 以外的鍵，依字元序）。
- `fields`：name、description、personality、scenario、first_mes、mes_example、creator_notes、system_prompt、post_history_instructions、creator、character_version；字串原文，缺或非字串為 null。
- `tags`、`alternate_greetings`：陣列裡的字串原文（不 trim、不濾空）。`extensions`：原樣，缺為 null。
- `openings`：first_mes 與 alternate_greetings 各自 trim 後的非空字串，依序。
- `route`：桌面版 `probe_import` 的 name（trim 後非空）、book_shaped、lorebook_heavy、book_entries、alternate_greetings；`decision`＝沒有 name 或 book_shaped 時 `worldbook`，否則 `ask`（由玩家選身分）；`suggested`＝decision 是 worldbook、或 book_shaped／lorebook_heavy／有備用開場白時 `worldbook`，否則 `character`。
- `books.character`：`character_book` 是物件時 `{name, form, entries}`，否則 null（角色卡路把它併進桌的世界書）。
- `books.worldbook`：世界書路會匯入的書（桌面版 `worldbook_json`）：`source` 為 `character_book`（有條目）、`top_level`（頂層有 `entries` 鍵）、`persona`（人設欄 description／personality／scenario／mes_example 非空者以空行併成一條常駐條目，comment＝卡名）、`none`（什麼都沒有）。
- `interface`：`extensions.regex_scripts` 裡啟用、非 promptOnly、placement 含 2 的顯示腳本（name、find_regex、replace_string、trim_strings、min_depth、max_depth）；`mvu`＝`tavern_helper`／`TavernHelper_scripts` 裡有啟用、內容含 `MagVarUpdate` 的腳本；`unsupported`：first_mes 或 description 含 `SCRYPT`（不分大小寫）＝`scrypt`（腳本清空），顯示腳本是吞整段的萬用式且替換字串短於 2000、含 `.load(` ＝`remote_loader`（腳本清空）。

## 有效性規則
- `validity.character`（角色卡路，桌面版 `parse_character`）：`name` 不是字串或 trim 後為空＝`card_missing_name`（空名字桌面版身分框不給走角色卡路）；trim 後含 `\n` 或 `\r`＝`name_not_single_line`；其餘 null。
- `validity.worldbook`（世界書路，桌面版 `worldbook_json`）：`books.worldbook.source` 是 `none`＝`card_nothing_to_import`，其餘 null。
- 網頁版只收角色卡路有效的卡（存檔要帶得回桌面版）；匯入身分只列有效的路。

## 條目
- `form`：`array`、`object`、`none`。
- 物件形一律算有條目（`has_entries`、`book_entries`、`lorebook_heavy`、桌面版 `import_worldbook` 同規則），照鍵的數字順序展開（卡片介面看到的順序；桌面版新配的 UID 改照 ST 載入順序，見 world-info 契約的 `bookOrder`），非數字鍵排最後依字元序；陣列形照原順序。
- 值不是物件的（字串、null、陣列）兩種形狀都不算條目、直接略過，匯入不得因此失敗。
- 每條：`key`（物件鍵或陣列索引字串）、`uid`、`keys`、`secondary_keys`、`comment`、`content`、`constant`、`enabled`、`order`、`position`（原樣，缺為 null）。
  - 陣列形（V2）：keys／secondary_keys／insertion_order／enabled（缺或 null＝true，其餘照 JS 真假值，與匯入同一套規則）／`id`。
  - 物件形（ST）：key／keysecondary／order／`!disable`／`uid`（缺則取鍵的整數）。
  - 整數欄只認 JSON 整數；字串清單只收字串項。
