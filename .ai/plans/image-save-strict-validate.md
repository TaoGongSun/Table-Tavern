# image-save-strict-validate：盤點、定案與施工做法

一～三節是施工前盤點。實測樣本：`TestCards/` 21 張真實角色卡 PNG 用 `validate_png_image`（8192／24M 像素）全過；JPEG→PNG 轉檔輸出（4096²）也過。盤點用的暫時測試已還原。

## 一、寫圖入口與現況檢查

| 入口 | 落點 | 現有檢查 | 位置 |
|---|---|---|---|
| 手動上傳大圖／頭像（含從圖庫挑圖重裁） | `characters/<id>.png`、`.avatar.png`（存檔時） | 前端 `<input accept=png/jpeg/webp>`→FileReader→CropDialog 用 canvas 重繪並 `toBlob("image/png")`（大圖寬≤1024、頭像 256²）；後端只驗 magic（`ImageNotPng`）＋角色存在，原子寫 | `CardEditor.tsx:292`、`CardImageDialogs.tsx:41-80`；`images.rs::save_character_png` 82-95 |
| AI 生圖（API／codex／agy／grok） | `gallery/<ms>.png` | 依 magic 認格式；PNG **原樣放行不驗證**；JPEG／WebP 解碼（寬高 ≤8192、配置 ≤512MB）重編 PNG；SVG 與未知格式拒收；下載 32MB／60s 上限 | `generated_image.rs::to_png`、`commands/image.rs::store_generated_image` |
| 圖庫挑圖 | 不寫檔，回前端進裁切 | 讀檔只看副檔名組 data URL，無驗證 | `commands/image.rs::read_gallery_image` |
| 重構卡匯入帶的圖 | 同手動上傳（呼叫 `save_character_image/avatar`） | decode 階段已用 `validate_png_image`（8192／24M）；落地時再過上列 magic 檢查 | `refactor/card_png.rs:192,317`、`apply.rs:464` |
| 角色卡 PNG 匯入 | `characters/<id>.png`（整個原檔） | 只驗 tEXt `chara`／`ccv3` 能解；逐 chunk 走但**不驗 CRC、zlib、IDAT 存在、IEND 後資料**；寫入不看圖本身 | `card.rs::import_character`、`card_io.rs::find_card_text` |
| 世界書 PNG 匯入的 GM 圖 | `worlds/<id>/gm.png` | 只驗 magic，失敗靜默（回 bool 被丟棄）；帶 regex_scripts 時另存原卡 `world_card.png` 也無圖檢查 | `images.rs::save_gm_image`、`interface.rs::save_world_card` |

沒有「背景圖」上傳入口（grep 無）。已用嚴格驗證的：重構卡 PNG 匯出（封面與素材，`card_export.rs`）。ST 角色卡匯出的底圖（`export.rs::export_base_png`）讀回存圖只驗 magic。

## 二、換成嚴格驗證後會被拒的輸入

驗證項：CRC、IHDR 組合、IDAT 連續、IEND 恰一個且後無資料、zlib 完整＋Adler、掃描線濾波、palette 索引、APNG、尺寸／像素上限（沿用卡片 8192 邊、24M 像素）。

| 情境 | 現在 | 嚴格後 | 玩家會碰到嗎 |
|---|---|---|---|
| 手動上傳任何圖 | 經 canvas 重繪，輸出必為瀏覽器編的合法 PNG | 全過（≤1024 寬） | 不會；canvas 這層已把壞圖、APNG、其他格式都洗掉，後端嚴格驗證對這條只是防線 |
| 手動上傳無法解碼的檔（選「所有檔案」挑了壞圖） | 前端 `Unable to load image` 英文原文顯示在裁切窗 | 不變（拒在前端，到不了後端） | 偶爾；但文案是既有問題 |
| AI 回 PNG 但壞（zlib 壞／截斷／CRC 錯／palette 越界） | 原樣存進圖庫，縮圖壞、挑圖後裁切失敗 | 拒收 | 低機率；API 串流截斷或模型亂湊 PNG 時可能 |
| AI 回 APNG | 原樣存，瀏覽器只顯示第一格 | 拒收 | 極低（主流生圖模型不輸出 APNG） |
| AI 回 JPEG／WebP，轉 PNG 後 >24M 像素（如 6000×5000；`to_png` 上限卻是 8192² =67M） | 存進圖庫 | 拒收（實測 6000×5000、8192² 都被擋，4096² 過） | 極低；目前生圖模型多在 1–4K。但兩邊上限不一致，要擇一統一 |
| AI 回 PNG 超過 24M 像素 | 存 | 拒收 | 同上 |
| AI 回 WebP／GIF-like 動畫 | WebP 取第一格轉 PNG | 不變 | 極低 |
| 角色卡 PNG 匯入：結構不嚴但瀏覽器能顯示（IEND 後尾隨資料、CRC 錯、缺 zlib 結尾） | 卡資料讀得到就匯入，整檔存成角色圖 | 若整檔套嚴格驗證則整張卡匯入失敗 | 中：21 張測試卡全過，但第三方工具編輯過的卡常有尾隨資料，這是最大誤傷面 |
| 角色卡 PNG 為 APNG／>24M 像素大圖 | 匯入 | 拒收 | 低，但原始卡圖有人用 4000×6000 以上 |
| 世界書 PNG 的 GM 圖不合格 | 現在存（或 magic 不對就略過） | 略過不存、匯入本身照常（目前就是靜默） | 低，影響只是 GM 沒圖 |
| 舊圖庫／舊角色圖 | 已存的壞圖 | 不回頭驗（嚴格驗證只加在寫入） | 讀取路徑不變 |

## 三、拒收時目前玩家看到的

- 後端 `save_character_image/avatar` 非 PNG：`ImageNotPng`→`be_image_not_png`「圖片必須是 PNG」，顯示在角色編輯頁頂端訊息列（`CardEditor.tsx:188` `setMessage(String(reason))`，儲存流程已先 `write_character` 成功，所以文字存了、圖沒存，無專屬提示）。
- 重構卡落地失敗：不報錯，`images_failed` 列角色名由前端說明「圖沒寫成」。
- 角色卡 PNG 結構壞（匯入）：`PngInvalid{detail}`→`be_png_invalid`「PNG 檔案損壞：{detail}」，detail 為英文技術原文；沒有 `chara` 資料 `CardPngNoData`。顯示在匯入對話窗。
- AI 生圖：`AI_IMAGE_UNSUPPORTED_FORMAT`／`AI_IMAGE_DECODE_FAILED`／`AI_IMAGE_DOWNLOAD_FAILED` 映射 `errImage*`（十語系），顯示在 AI 生圖對話窗，文案固定含「這張沒有存進圖庫」。嚴格驗證失敗目前沒有對應碼，要新增或重用 `AI_IMAGE_DECODE_FAILED`（「打不開，可能檔案不完整…再生一次」，對壞 PNG 語意正確，對 APNG／超大不精準）。
- GM 圖：無提示。

## 四、定案

1. AI 生圖拒收沿用 `AI_IMAGE_DECODE_FAILED`，不加新碼；2、3 自動處理後，拒收只剩真壞檔。〔作者裁決 2026-10-07〕
2. AI 回 APNG：保留檔內預設靜態圖（IDAT）存成靜態 PNG；預設圖不屬動畫時存的是替代圖，接受。〔作者裁決 2026-10-07〕
3. AI 圖超過像素上限：自動縮到上限內再存；`to_png` 8192 邊與嚴驗 24M 像素統一成同一組上限。〔作者裁決 2026-10-07〕
4. 看似 PNG 但結構壞：不容錯修復，拒收。〔作者裁決 2026-10-07〕
5. 角色卡 PNG 匯入圖過不了嚴驗：卡照常匯入，圖改存重編後的乾淨 PNG；重編也救不回就卡照匯、不存圖，匯入結果提示圖沒存。〔作者裁決 2026-10-07〕
6. 世界書 PNG 的 GM 圖不合格：比照角色卡先重編救回，救不回才略過並在匯入結果提示。〔作者裁決 2026-10-07〕
7. 角色編輯儲存：圖先驗，驗證失敗整個儲存取消、編輯狀態保留。〔作者裁決 2026-10-07〕預驗通過後寫檔 IO 失敗：回報錯誤、草稿保留，文字可能已存。〔作者裁決 2026-10-07〕
8. 手動上傳：後端加嚴驗當防線、不加新提示；裁切窗英文錯誤原文換成十語系翻譯字串。〔作者裁決 2026-10-07〕
9. 一包做完；讀取路徑（圖庫挑圖、`export_base_png`、已存舊圖）不回頭驗；`world_card.png` 原樣存、不套圖檢查。〔作者裁決 2026-10-07〕

## 五、施工做法

### 共用：上限、重編、搬卡資料

- `import/png_image.rs`：
  - 加 `STORED_IMAGE_LIMITS`（8192 邊、24M 像素），`CARD_LIMITS.pixels` 改指它；全 app 存圖只認這一組（定案 3）。
  - 加 `REENCODE_SOURCE_LIMITS`（16384 邊、64M 像素）：要縮圖的來源上限。64M×8 位元組（RGBA16）≈ 512 MB，與解碼器 `max_alloc` 對齊；索引色列緩衝隨寬度，最寬 16384 也只有幾十 KB；寬高在 IHDR 就擋，配置前不會碰到相乘溢位。超過一律當救不回。
  - `validate_png_image` 錯誤改回分類 enum：`Animated`／`TooLarge{w,h}`／`Invalid(String)`，實作 `Display`，既有呼叫端（`card_png`、`card_export`）照舊拿字串。
- 新增單檔 module `import/png_clean.rs`（函式都吃 `ImageLimits`，測試可用小上限）：
  - `strip_animation(bytes)`：原檔先過既有 `for_each_chunk` 的完整結構／CRC 檢查（IDAT 連續、恰一個 IEND、IEND 後無資料）；過了才丟掉 `acTL`／`fcTL`／`fdAT`、其餘原樣串回，留下的就是 IDAT 預設圖（定案 2）。剝後位元組再走嚴驗。`IDAT → fcTL → IDAT` 這種原本不連續的檔，在剝除前就被擋，不會被洗成合法。
  - `fit_size(w,h,limits)`：等比縮放後取整，再逐步縮小較長邊直到兩邊 ≤ max_side 且 w×h ≤ max_pixels，各邊至少 1。重編輸出嚴驗若仍回 `TooLarge`，再縮一階重來，不會因取整誤差改走拒收。
  - `reencode(bytes, format, limits)`：
    - 用 `ImageReader::with_format(..).into_decoder()` 先只讀檔頭拿寬高，以 `REENCODE_SOURCE_LIMITS` 同時查邊長與像素數，超過就回錯、不解碼。
    - 過了才完整解碼（另設 `max_alloc` 512 MB 當第二道）→ 超出 `limits` 就 `resize`（CatmullRom）→ 編 PNG → 嚴驗輸出。
    - PNG、JPEG、WebP 共用這一支，同樣尺寸縮出同樣結果。
    - 解碼器（image crate 預設行為）解不開的就算救不回，不另開容錯選項。
  - `transplant_card_text(original, clean)`：自己逐 chunk 掃 original（不用 `for_each_chunk`，它遇到 IEND 後尾隨就整個失敗），掃到 IEND 或遇到不完整的 chunk 就停；IEND 前 keyword 為 `chara`／`ccv3` 的 tEXt 照原順序用 `png_chunk` 重算 CRC，插在乾淨圖 IEND 前。
- 兩支流程，回傳 `Stored` = `AsIs`（傳入那份本來就合格，呼叫端存自己手上的位元組）／`Rewritten(Vec)`（截到 IEND 或剝掉動畫後合格，要存的是帶回的這段）／`Reencoded(Vec)`。截斷或剝除過的結果一律帶位元組回來，呼叫端不會把尾隨寫回去（尾隨卡存下內容等於截到 IEND 的那條驗收把關）：
  - **嚴格版 `strict_stored_png(bytes)`（AI 用）**
    1. 有動畫 chunk 先 `strip_animation`，剝不了就回錯。
    2. 以 `STORED_IMAGE_LIMITS` 嚴驗，`Ok` 存。
    3. `TooLarge`：以 `REENCODE_SOURCE_LIMITS` 嚴驗（結構仍嚴格，只放寬尺寸），過了才 `reencode` 縮圖。
    4. `Invalid` 回錯（定案 4）。
    - 只有過大的圖會解壓兩次，工作量受來源上限約束。
  - **救圖版 `salvage_stored_png(bytes)`（角色卡、GM 用，定案 5、6）**：能保留原位元組就不重編。
    1. 用與 `transplant_card_text` 同一套自掃把原檔截到 IEND 為止（`truncate_at_iend`）；掃不到完整 IEND 就不截、用原檔。
    2. 截斷後的位元組走嚴格版（含動畫剝除、過大縮圖），成功就用。IEND 前的 `chara`／`ccv3` 還在，不必搬也不重編。
    3. 仍失敗（`Invalid`、剝動畫失敗，或 `TooLarge` 但結構不合格），就對截斷後的位元組 `reencode(STORED_IMAGE_LIMITS)`。解碼器略過動畫輔助 chunk，只取預設圖。
    4. 重編失敗或超過 `REENCODE_SOURCE_LIMITS` 才算救不回。
    - 過不了嚴驗的檔不先跑寬鬆嚴驗，直接交給已有尺寸前檢的 `reencode`。
  - AI 路徑不截斷：IEND 後尾隨維持 `AI_IMAGE_DECODE_FAILED`。
  - 解碼器行為實證（2026-10-07，image 0.25.10＋png 0.18.1，與專案 `Cargo.lock` 同版本，scratch 專案實跑）：
    - 索引色 1／2／4／8 位元、灰階 1／2／4 位元都解得開（`Transformations::EXPAND` 先展開成 8 位元；`PngDecoder::new` 裡 `Indexed` 與低位元深度那幾支回 unsupported 的分支比對的是展開後的輸出格式，實際走不到）。TestCards 唯一的 color type 3 卡 `donass.png` 原檔與加尾隨後都解得開。
    - 輔助 chunk（tEXt）CRC 壞照樣解得開；IDAT CRC 壞解不開。
    - palette 索引越界不報錯，越界像素解成黑色。
    - 因此「索引色且 IDAT 有問題」不需另開分支，跟其他色型一樣：IDAT 壞（zlib、CRC、截斷）→ 救不回；只有越界索引 → 重編成越界處為黑的圖再存（嚴驗會擋原檔，救圖版接受這個結果）。

### AI 生圖（`generated_image.rs::to_png`）

- PNG 走嚴格版 `strict_stored_png`；錯誤回 `AI_IMAGE_DECODE_FAILED: <detail>`（定案 1、4）。
- JPEG／WebP 走 `reencode(STORED_IMAGE_LIMITS)`；刪掉 `MAX_DIMENSION`，來源上限改用上面那組。
- 檔頭註解改寫成「統一成通過嚴驗的 PNG」。

### 手動上傳與儲存順序（定案 7、8）

- `images.rs::save_character_png`：magic 檢查換成 `validate_png_image(STORED_IMAGE_LIMITS)`；不是 PNG 仍回 `ImageNotPng`，其餘回既有 `PngInvalid{detail}`（不加新文案）。重構卡落地 `apply.rs` 走同一支，輸入已用同一組上限驗過，結果不變。
- 新 command `check_character_image(data)`：只跑同一支驗證、不寫檔。
- `CardEditor.tsx` 儲存：
  - 開始時拍下卡與兩張草稿的快照，`write_character` 前對每張待存草稿圖 `check_character_image`；任一張失敗就顯示訊息並 return，零寫入，欄位與草稿都不動。
  - 儲存進行中再按儲存不會再開一輪。
  - 編輯輪次 generation：換卡（含換走再換回同一張）、換草稿就遞增；儲存開始記下輪次。
  - 預驗回來時輪次變了就整輪放棄：不寫檔、不改畫面。
  - 寫檔回來時輪次變了：只重讀圖（磁碟已寫），草稿、欄位、訊息一律不動；沒變才清草稿、更新欄位與「已儲存」。重讀圖之後另用只隨換桌、換卡遞增的編輯對象輪次判斷要不要送 onSaved（清草稿也會遞增 generation，不能再用它）。只比 characterId 分不出 A→B→A，晚回會蓋掉新編輯或清掉別張卡的草稿（Sol 驗收反例，已納入測試）。
  - 「儲存進行中」鎖在每一輪結束時一律放開（`finally`），包括放棄、預驗失敗、IO 失敗。`CardEditor` 換卡不重掛元件，鎖不放開會讓之後的卡都存不了。
  - 預驗通過後寫檔 IO 失敗：顯示錯誤、草稿保留（文字可能已存，定案 7 後半）。
- `CardImageDialogs.tsx`：`Unable to load image`、`Unable to create image canvas`、`Unable to crop image` 換成翻譯鍵（載入失敗一句、裁切失敗一句），訊息不帶 `Error:` 前綴。十語系：de en es fr ja ko pt-BR ru zh-CN zh-TW。

### 角色卡 PNG 匯入（定案 5，`card.rs::import_character`）

- 救圖版 `salvage_stored_png`：
  - `AsIs` → 原檔照存 `characters/<id>.png`。
  - `Rewritten`（截斷或剝動畫後合格，原位元組的子集，tEXt 原樣在）→ 不搬，直接核對。
  - `Reencoded` → 先 `transplant_card_text` 再核對。
  - 核對：最終 PNG 再嚴驗，而且 `decode_png_character(最終檔)` 要等於匯入時解出的 JSON；兩項都過才存 `.png`。`chara` 只在 IEND 後的卡，截斷後讀不回同一份 JSON，走下面的退路。
- 失敗退路：任一步失敗（救不回、搬移失敗、最終檔不合格或讀回不同）→ 不寫 `.png`，改把已解出的卡 JSON 存成 `characters/<id>.import.json`（`read_card_interfaces` 已會退讀），回報 `image_dropped`。搬移錯誤一律走退路，不新增整卡匯入失敗的條件。
- 落檔：`.png` 與 `.import.json` 只寫其一，改用 `commit_world_write_atomic`。寫入 IO 失敗照現行 `?` 回錯，未完成標記留著（現有語意）。
- `image_dropped` 一路往上送：`import_character` → `Imported` 加 `image_dropped` → `import_character_file` → command `CharacterImport.image_dropped`。前端不論卡有沒有隨身世界書，`image_dropped` 為真就另跳一句「卡片圖沒存，之後可自行上傳」（十語系）。

### 世界書 PNG 的 GM 圖（定案 6）

- `save_gm_image` 改回 `DataResult<GmImage>`，`GmImage` = `Saved`／`NotPng`（純 JSON，不動舊圖）／`Dropped`（PNG 但救不回）。
  - 判定走救圖版 `salvage_stored_png`，重編只寫像素，不搬 `chara`，不動 `world_card.png`。
  - 寫入用 `commit_world_write_atomic`；IO 失敗回 `Err`，`import_worldbook_file` 照「匯入本身失敗」處理，未完成標記留著，不當成略過。
- `Imported.image_dropped` 由 `GmImage::Dropped` 設定 → `WorldbookImportResult.image_dropped` → 前端完成訊息後接「封面圖沒存」一句（十語系）。

### 驗收（全部必要）

後端單元：
- 上限：
  - 極寬索引色 PNG（如寬 10 萬、高 1）在 IHDR 就被擋、不配置列緩衝。
  - 超過 `REENCODE_SOURCE_LIMITS` 的 PNG 當救不回。
  - JPEG／WebP 在完整解碼前就擋：灰階 JPEG 與 WebP 各一張邊長合格、像素數超過 64M 的反例（測試用小上限模擬），確認沒進完整解碼就回錯。
- `fit_size`：取整後同時符合邊長與像素上限（剛好 24M、單邊超、極細長、1 像素邊）；同尺寸 PNG／JPEG／WebP 縮出相同寬高。
- APNG 兩種布局：預設圖屬動畫（`fcTL` 在 IDAT 前）與不屬動畫，IDAT 與 fdAT 用不同像素，確認存下的是 IDAT 像素且無 `acTL`／`fcTL`／`fdAT`。嚴格版還要拒收這三種：CRC 合法但 IDAT zlib 壞的 APNG、`IDAT → fcTL → IDAT`（剝除前就被結構檢查擋下）、APNG 帶 IEND 後尾隨資料。
- 救圖版：
  - IEND 後尾隨（含索引色＋尾隨）→ 存的是截到 IEND 的原位元組（byte 相同、沒重編）。
  - tEXt 等輔助 chunk CRC 壞、壞 APNG 結構但預設圖解得開 → 重編後過嚴驗。
  - 只有 palette 越界 → 重編後過嚴驗。
  - IDAT CRC 壞、zlib 壞 → 救不回。
  - 不為了讓測試通過開忽略 CRC。
- AI 壞 PNG（截斷、壞 CRC、IEND 後尾隨、palette 越界、合法 CRC 但 zlib 壞）→ `AI_IMAGE_DECODE_FAILED`；過大 PNG／JPEG（小上限）→ 縮到上限內且過嚴驗。
- `transplant_card_text`：
  - chara 與 ccv3 照序搬，讀回同一份 JSON。
  - IEND 後有尾隨垃圾或不完整 chunk 時照樣成功。
  - chara 只在 IEND 後 → 搬不到 → 走 `.import.json`，整卡仍匯入成功。
- `images.rs`：結構壞的 PNG 回 `PngInvalid`；`check_character_image` 不寫檔。
- 角色卡匯入：
  - 合法卡原檔 byte 相同。
  - IEND 後尾隨的卡 → `.png` 過嚴驗、無 `.import.json`，內容等於截到 IEND 的原檔。
  - 索引色＋尾隨的卡（以 `donass.png` 這類 color type 3 為底）→ 同上，介面還在。
  - 圖救不回的卡（保留既有 `minimal_png` 這類無 IDAT 假圖的退路斷言）→ 無 `.png`、有 `.import.json`、`image_dropped`。
  - 三種情況 `read_card_interfaces` 的 scripts、mvu、opening 都和原檔一致。
- GM：
  - 合法 → 原樣存。
  - 尾隨 → `gm.png` 存截到 IEND 的原位元組；過大或 tEXt CRC 壞 → 重編存 `gm.png`。兩種都要過嚴驗，`world_card.png` 仍是原檔位元組。
  - 救不回 → 無 `gm.png`、`image_dropped`。
  - 故障注入：在既有 `gm.png` 的桌上跑 `import_worldbook_file`，沿用 `world_file.rs` 的注入機制只打 GM 這次寫入（`begin` 已成功之後）。
    - `WriteFailGuard::partial_ending("gm.png.tmp", 1)`：tmp 寫一半失敗。
    - `RenameFailGuard::fail_ending("gm.png", 1)`：只打 GM 那次替換（`world_file.rs` 的改名注入新增依目標路徑篩選，比照 `RemoveFailGuard::fail_ending`）。
    - 兩種都要確認：回錯、舊 `gm.png` 位元組不變、無殘留 `.tmp`、未完成標記仍在。
- 既有拿 `minimal_png` 或 `b"\x89PNG...first"` 當圖、斷言存成功的測試（`card_io`、`receipts`、`export`）改用 `test_png::real_png` 組的合法卡；斷言救不回退路的保留原假圖。

前端單元：
- `CardEditor`：大圖合法、頭像非法 → `write_character` 呼叫 0 次，欄位與兩張草稿都保留。
- `CardEditor`：受控 promise 卡住預驗，期間重複按儲存只跑一輪；換卡、換草稿後放行舊預驗 → 不寫檔、新草稿不被清、畫面不被改，而且之後對新卡／新草稿按儲存能正常存（鎖已放開）。預驗失敗、IO 失敗後也能再存。
- `CardEditor`：預驗通過、寫圖失敗 → 顯示錯誤、草稿保留。
- `CardImageDialogs`：載入失敗顯示翻譯字串。
- 匯入：沒有隨身世界書的卡 `image_dropped` 也會跳提示；世界書 `image_dropped` 接在完成訊息後。i18n 十語系齊全（verify 既有檢查）。

測試通道實測（`harness:build`）：
- TestCards 帶介面的卡與 `donass.png`（索引色）各加 IEND 後尾隨再匯入：圖看得到，介面、MVU、開場白照常。
- 無 IDAT 假圖卡匯入：有提示，介面照常。
- 世界書 PNG 兩條：救回（加尾隨）→ GM 圖看得到；救不回 → 有提示。
- 手動上傳裁切存檔照常；頭像非法無法從 GUI 造，交給單元測試。
- AI 生圖用最低檔實送一張，確認仍存得進圖庫（耗額度小）。
