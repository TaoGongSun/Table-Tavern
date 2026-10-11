# character-image-overwrites-card 方案

## 根因

`characters/<id>.png` 一檔兩用：
- 匯入時 `store_card_source` 把救過的原卡 PNG（帶 chara／ccv3）寫進這檔，存不下才改寫 `<id>.import.json`（`import/card.rs` store_card_source）。
- 同一檔也是顯示圖：卡片編輯器換圖 `save_character_image`、刪圖 `delete_character_image`（`import/images.rs`）。

換一次圖或刪圖，原卡文字（介面腳本、MVU、隨身世界書原文）就沒了；`read_card_interfaces` 讀 `<id>.png` 找不到卡片資料，整張卡被當成非匯入卡略過。反過來，玩家把另一張 ST 卡 PNG 設成角色圖，介面會讀到那張卡的腳本。

重構套用寫 Portrait（`refactor/apply.rs` 約 499）一律寫在新建的角色上，不蓋原卡；實際風險是舊版重構卡的 Portrait 素材夾帶 chara，會讓新角色冒出多餘介面——本案一併堵住。

## 盤點

當**原卡**讀的：只有 `import/interface.rs` `read_card_interfaces_raw` 角色層。下游：`chat_assembly.rs`（卡內 regex）、`mechanism/policy.rs`（MVU 數字規則）、`world_scan/book.rs`（MVU 判定）、`commands/refactor.rs`（重構判介面）、前端卡片介面面板。

當**顯示圖**讀寫的：`import/images.rs`；前端 `useCharacterController.ts`、`useTableCover.ts`、`CardEditor.tsx`；ST 卡匯出底圖（`import/export.rs` export_base_png，內容由現行卡重建）；重構卡匯出封面與 Portrait 素材（`refactor/card_export.rs`）；重構套用 Portrait。頭像 `<id>.avatar.png` 本來就獨立。

不受影響：世界層原卡 `source-card.*`、GM 圖 `gm.png`；web/ 不存角色檔，網頁存檔的 `card_png` 匯進桌面走同一條 `import_character_placing → store_card_source`。

## 修法（已實作）

1. **原卡獨立**：`store_card_source` 寫 `<id>.card.png`（與 `.import.json` 二擇一，救圖＋嚴驗＋讀回比對照舊；不存匯入原始 bytes，免得大型 APNG 原樣落地）。之後沒有路徑會寫它。
2. **顯示圖另存**：PNG 卡只要救得回圖，就把救回的圖剝掉 chara／ccv3 寫成 `<id>.png`；搬卡文字失敗、原卡退成 `.import.json` 時也照存。`image_dropped` 只表示「圖救不回、沒有顯示圖」。
3. **剝 chunk 共用**：`import/png_clean.rs` 的 `is_st_card_chunk`、`copy_chunks_before_iend`（不驗 CRC、錯誤種類同舊 embed_chara_chunk、IEND 後尾巴丟掉）、`strip_card_text`；`export.rs` embed_chara_chunk、`refactor/card_png.rs` stripped_base（先過嚴格結構檢查再剝）、匯入顯示圖、`save_character_png` 四處共用。
4. **存角色圖也剝**：`save_character_png`（角色圖與頭像）存檔前剝掉卡文字，玩家換圖、重構 Portrait 都不會把別張卡的資料存進來。
5. **讀取點**：角色層只讀 `<id>.card.png`、`<id>.import.json`；`.card.png` 讀出一般圖（結構完好、沒卡片資料）也算 unreadable。世界層略過一般圖的語意不變。
6. **失敗語意**：`.card.png`／`.import.json`／顯示圖任一寫入失敗 → 整次匯入回錯，並以 `delete_character` 收掉剛建的 md 與已寫檔（不留半匯入角色）。例外：回滾本身也失敗時留得下殘檔，此時回 `CharacterImportRollbackFailed`，同時帶原寫入錯誤、角色 id 與殘留檔名（含暫存檔），請玩家刪掉該角色或到桌資料夾 characters 手動刪。網頁存檔匯入與重構清回失敗時整張新桌／臨時根本來就整批收掉，呼叫端以 `original_import_error` 換回原錯。網頁存檔匯入把 `image_dropped` 帶回 `WebSaveImported`，前端完成訊息加「卡圖沒存成」。
7. **刪除**：`data::delete_character` 先刪 md（失敗即停），再逐一清圖庫、`.png`、`.avatar.png`、`.card.png`、`.import.json` 與匯入暫存檔（`.card.png.tmp`／`.import.json.tmp`／`.png.tmp`），單檔失敗不擋其他檔，最後以 `CharacterFilesLeft`（件數＋相對桌資料夾的位置：`characters/<檔名>`、`gen-gallery/<id>/`）彙總；可重做。匯入回滾也靠它清暫存檔。撤銷匯入（含手動撤銷）刪角色失敗一律回錯（附檔殘留報 `UndoFilesLeft`，其他報 `UndoIncomplete`，都說再按一次撤銷會重試）、收據不彈出，玩家再按一次即可清乾淨；多角色撤銷先讀齊所有角色名再刪。原本單獨補刪 `.import.json` 的程式拿掉。

## 既有資料

舊桌不遷移〔主線裁定，依作者裁決：正式發佈前舊桌不相容可不管（memory no-legacy-table-compat）〕。舊桌 PNG 匯入的角色只有 `<id>.png`、沒有 `.card.png`，改版後：
- 卡片介面面板不再顯示該卡介面；
- 卡內 regex（chat_assembly）不再套用；
- MVU 數字規則（mechanism/policy）與 world_scan 的 MVU 判定不再認這張卡；
- 未重構的舊桌拿去重構，會被當成無介面、非 MVU 的卡。

開桌不報錯；重構清回照匯入原檔重匯，不受影響（清回後兩檔都在）。JSON 匯入的角色（`.import.json`）不受影響。要恢復請重新匯入。

## 測試（已補）

- 匯入：有效／截尾／重編 PNG 寫 `.card.png`＋剝過的 `.png`；救不回只寫 `.import.json` 並回 `image_dropped`；chara 在 IEND 後（圖救得回、卡文字搬不到）→ `.import.json`＋顯示圖、不算 dropped；JSON 卡不產生 `.png`／`.card.png`。
- 失敗矩陣：`.card.png`／顯示圖／`.import.json` 各自改名失敗與寫到一半（`RenameFailGuard::fail_ending_after`、`WriteFailGuard::partial_ending_after`）→ 回錯、角色目錄清空（含暫存檔）。
- 雙故障：顯示圖改名失敗＋回滾時 `.card.png` 刪不掉、或 `<id>.png.tmp` 刪不掉 → `CharacterImportRollbackFailed` 帶原錯、id、殘檔名（含暫存檔）。
- 重構清回重匯時顯示圖寫失敗 → 整次清回失敗、原桌原封不動；再加暫存檔刪不掉 → 回原錯不回 rollback 訊息；排除故障後重試清回成功。
- 網頁存檔匯入：顯示圖寫失敗＋暫存檔刪不掉 → 回原錯不回 rollback 訊息。
- 換圖成另一張 ST 卡再刪圖：`.card.png` 不變、介面仍是原卡、顯示圖無卡資料。
- `.card.png` 是一般圖或截斷 → unreadable；`.card.png`／`.import.json` 組合。
- 重構套用夾帶 chara 的 Portrait → 顯示圖已剝、介面不多一份。
- 重構清回後 `.card.png` 與 `.png` 正確。
- `delete_character` 單檔刪除失敗 → 其他照清、回錯點名、重刪清乾淨；撤銷時刪卡失敗 → 收據不彈出、名單不動，重按清乾淨。
- 網頁存檔：一致 PNG 不 dropped；救不回 PNG 回 `image_dropped` 且前端完成訊息提示。
- `export/tests.rs`、`card_png` 既有測試照舊綠。

## 已知限制

- 殘檔一直刪不掉時，撤銷那筆匯入會一直回錯卡住，玩家要自己到桌資料夾清檔〔主線裁定，依作者裁決：罕見邊際狀況輕防〕。
- 玩家手動刪角色時 md 已刪、附檔刪失敗：回錯點名殘檔，但角色已不在清單上、沒有重刪入口，留孤兒檔〔主線裁定，依作者裁決：罕見邊際狀況輕防〕。
- 多角色撤銷刪到一半失敗再重按：前一次已刪掉卡檔的角色讀不到名字，重按的報告只列這次才刪掉的角色〔模型判斷·未裁決〕。

## 範圍外

- 頭像、GM 圖、世界層 `source-card.*` 的存法〔模型判斷·未裁決〕。
- 網頁版（web/）不存角色檔，無改動〔模型判斷·未裁決〕。
- 從收據原檔救回舊桌被蓋掉的原卡〔主線裁定，依作者裁決：memory no-legacy-table-compat〕。

## 已定

1. 原卡檔名 `<id>.card.png`〔主線裁定，依作者裁決：檔案落點由我決定（memory file-placement-is-my-call）〕。
2. 舊桌不遷移〔主線裁定，依作者裁決：memory no-legacy-table-compat〕。
3. 匯入時另存剝掉 chara／ccv3 的顯示圖 `<id>.png`，理由：做法單純、刪圖不需「已刪」記號〔主線判斷·未經作者裁決〕。
4. 顯示圖寫入失敗整次匯入回錯、不吞成 `image_dropped`〔主線判斷·未經作者裁決〕。
5. 撤銷時刪卡失敗改回錯、不彈出收據〔主線判斷·未經作者裁決〕。
