# 桌檔契約 v1：網頁存檔

網頁版的一桌存成一份 JSON，桌面版照它開一張新桌接著玩（單向，D15）。網頁版寫、桌面版讀；兩邊的結構檢查一致：網頁版與桌面版前端用 `web-save.ts` 的 `parseWebSave`，桌面版匯入器用 `src-tauri/src/import/web_save/`（另驗卡在匯入身分那條路的有效性與變數表的細部上限）。範例檔就是本目錄的 fixture（`minimal.json` 是最小合法存檔；`web-export.json` 是網頁版真匯出的存檔，由 `web/e2e` 帶 `TT_WEB_EXPORT_OUT=<路徑>` 跑一次另存，桌面版來回測試讀它），兩邊的測試都讀；`invalid/` 是共用負例：每份都是從 `minimal.json` 改一處，檔名前綴是兩端都要回的錯誤分類（`version--`＝版號不認得、`invalid--`＝不合契約）。改契約＝兩邊與 fixture 同一筆 commit 一起改。

## 外框
- `format`：固定 `"table-tavern-web-save"`。不是這個字串就不是網頁存檔。
- `version`：整數，目前只有 `1`。讀到別的值（含字串 `"1"`）一律以「版號不認得」拒收並說明，不猜；缺鍵算不合契約。
- `exported_at`：匯出時間（RFC 3339 字串）。
- 檔案上限 64 MiB；不認得的頂層鍵略過。
- 時間欄位（`exported_at`、每則 `ts`）：RFC 3339（日期、`T`、時分，秒與 1–9 位小數可省，`Z` 或 ±hh:mm），而且日曆上存在：月 1–12、日照當月（閏年 2 月 29 日）、時 0–23、分秒 0–59、偏移時 0–23 分 0–59。
- 整份（含原卡外殼）任何字串或鍵不得含孤立的代理字元（`\ud800` 這種跳脫），兩端都整份拒收。
- 欄位的有無：「可省」的欄鍵不在＝沒有，鍵在就不能是 `null`；「可為 null」的欄鍵一定要在，值可以是 `null`。兩種都不是的欄鍵必須在、也不能是 `null`。
- 金鑰、模型設定一律不進存檔。

## 一、卡
- `card`：原卡 JSON 外殼，一個字都不改（含 `spec`、`spec_version`、未知欄位）。
- `card_png`：可省，原 PNG 的標準 base64（補齊 `=`、結尾位元為零、不含空白）。與 `card` 不一致時以 `card` 為準（桌面版只在 PNG 讀出的卡與 `card` 完全相同時才用 PNG 匯入，好帶上卡圖）。
- `import_route`：`"character"` 或 `"worldbook"`，網頁版匯卡時實際採用的身分。桌面版照它走、不重新推測；該路桌面版會拒收的卡整份拒收（規則見卡片契約的有效性）。
- `regex_allowed`：玩家是否允許卡內 regex 腳本（D21）。
- `user_name`：玩家名（單行，可空）。
- `opening_index`：可為 null，玩家選的開場白序號（first_mes 為 0），沒放開場白是 `null`。

## 二、逐字稿
`messages` 依時間先後，每則：
- `id`：網頁版配的穩定 ID（1–128 字元，整份不重複）。
- `role`：`"user"` 或 `"char"`。
- `text`：存檔的文字（玩家句是過完送模前 regex 與巨集後的字；開場白保留巨集原文）。
- `raw`：可省，模型原文（與 `text` 相同時不寫）。
- `ts`：落進逐字稿的時間，RFC 3339。
- `opening`：可省（預設 false），只能是第 0 則、`role` 必須是 `char`。
- `interrupted`：可省（預設 false），回應中途停止或被截斷。
- `message_vars`：可省，這一則完整的 MVU 變數表（物件）；有任何一則帶表時 `mvu.seed` 必須是物件。

欄名語意取桌面版 `TranscriptEvent` 子集。

## 三、世界書觸發狀態
`world_info`：
- `entries`：`[{ "id": <穩定 ID>, "key": <卡片契約裡該條目的 key> }]`。桌面版匯入時內容重複被略過的條目，映到桌上保留的那一條的 UID。穩定 ID 由網頁版匯卡時配發；`key` 指向匯入身分那條路會匯入的書（角色卡路＝`books.character`，世界書路＝`books.worldbook`）。
- `timed`：計時狀態（sticky／cooldown／delay），欄位名照 ST，以條目穩定 ID 為鍵，以「訊息則數」計時。內容語意包 5 定，v1 只固定位置。
- `last_message_id`：可為 null，計時已算到的最後一則訊息 `id`，`null`＝還沒算過。
- `message_effects`：以訊息 `id` 為鍵，記那則被移除或改寫時它造成的計時變化能否回退。內容語意包 5 定，v1 只固定位置。

桌面版只保存、不消費：原樣存旁檔，另附「穩定 ID → 桌面 UID」與「訊息 id → 桌面事件 id」兩張映射表；不把 sticky 條目改成 constant。

## 四、MVU 變數
`mvu` 可為 null（這桌沒有卡片變數）。否則：
- `macros`：可為 null，`{ "user": <字串>, "char": <字串，可為 null> }`，啟用當下的 `{{user}}`／`{{char}}` 代換值。
- `seed`：可為 null，開場那張完整表（物件），`null`＝尚無表（`{}` 是空表，不等於尚無表）。
- `layers`：六層全列，空的也留：`chat`、`character`（這張卡自己的那層）、`global`、`preset` 各是一張表（物件）；`script`、`extension` 是 `{ <原 ID>: <表> }`（原 ID 1–256 字元）。
- 每張表的上限與卡片變數寫入同一組（`src/shared/contracts/vars-table.ts`、`src-tauri/src/data/message_vars/json.rs`）：存檔裡的表整張以 `JSON.stringify` 緊湊寫法算 UTF-8 ≤ 2,097,152 位元組（原文的空白不算；數字照 JS 的寫法計長，超過 2^53 的整數先變成最接近的浮點數；解析前原文另擋 8 MiB）。卡片寫入有原文，原文位元組或緊湊寫法任一不超過就收（舊版只量原文，只放寬不收窄）。邊界案例兩端共用 `src/shared/contracts/vars-table-boundary.json`。其餘上限：深度 ≤ 32、單一字串 ≤ 65,536 位元組、陣列或物件的子項 ≤ 10,000、整張節點 ≤ 200,000、鍵 1–256 字元。

有效表選取（桌面版照這三條落地）：
1. 開場：用 `seed`。
2. 之後：依位置最新一則帶 `message_vars` 的訊息那張表；沒有就用 `seed`。換幕後的新幕以舊幕的有效表當種子。
3. 跨桌層（global、preset、extension）匯入時只補缺、既有鍵值不覆蓋（D18）；沒寫進去的部分以存檔裡仍在為準，不算丟。

`card_storage`：卡片介面的 localStorage，`{ <鍵>: <字串> }`，整份 JSON 不超過 64 KiB。

## 網頁版匯出（現況）
- 存檔在瀏覽器的 IndexedDB（`web/src/features/saves/`），一桌一格，存的就是這份契約；匯出＝原樣下載，匯入＝過同一個 `parseWebSave` 再收進存檔庫。
- ST 聊天變數就是 MVU 的兩層：這段對話的 local 寫進 `mvu.layers.chat`、跨對話的 global 寫進 `mvu.layers.global`；其他層、`seed`、`macros`、每則的 `message_vars` 等包 6（卡片介面）才產生，在那之前是空表或 `null`。
- `world_info` 包 5（世界書觸發）之前一律空結構；`card_storage` 包 6 之前是 `{}`。
- 網頁版讀進來但還用不到的欄位（觸發狀態、MVU 其他層、`message_vars`、`card_storage`、`raw`）與時間原文都原樣收著、再匯出原樣帶回；網頁版自己新增的則寫成 UTC。存檔沒有 `mvu`、這桌也沒寫出變數就照樣寫 `null`。
- global 層：存在瀏覽器（存檔庫的 `globals`），所有存檔共用、重新整理不丟（D29）。開站讀不到就提示可重試；各桌的寫入在取樣當下預約版本與寫入資格，預約時還沒讀回過的那筆永遠不寫（重試成功後也不放行），排同一條隊、舊版本不蓋新版本。從存檔接著玩時只補這個分頁沒有的鍵（同 D18）；這一格匯出時只寫「存檔原有的鍵＋這桌玩的期間寫過或刪過的鍵」（照寫入紀錄，寫回原值也算，寫入失敗不算），分頁裡別桌的鍵不混進來；這桌沒碰過的鍵（含分頁已有別的值、沒落地的）照存檔原值帶回。
- 自動存檔只在回合結束後寫（D27：新開的桌第一次開口時先把開口前的樣子存成一格，寫成了才送模，存不進去就不送、原句留在輸入框）；回合中重新整理就退回上次完整回合，進行中的玩家那句記在 sessionStorage，接著玩時放回輸入框（D28）。
- ST 聊天檔匯出（D3）另給原卡檔：從 PNG 匯入的給原 PNG，其餘給原卡 JSON 外殼。
- 桌面版目前只把 ST 聊天變數落進 chat 層檔案：沒有 MVU 種子就不進變數模式，下一輪提示讀桌狀態、不讀 chat 層；桌面版巨集也只換 `{{user}}`／`{{char}}`。變數進提示等包 6〔模型判斷·未裁決〕。

## 桌面版匯入（落地）
- 一律開新桌（桌名＝卡名）；身分照 `import_route`。不記匯入收據（不能「復原上次匯入」）。
- 玩家名非空時建一張同名玩家卡。
- 逐字稿寫進第 0 幕：玩家句→玩家事件；開場白→GM 旁白（`opening`）；角色卡路的回覆→該角色發言，世界書路的回覆→GM 旁白。事件 id 由桌面版重配。
- 世界書：角色卡路的卡內條目，原卡沒指定 `extensions.table_tavern.visibility` 的設成只有這個角色看得到（D16）；明示的照舊；世界書路沒有角色，照桌面版預設給 GM。
- MVU：`seed` 是物件就把控制檔設成變數模式、這一幕新 epoch 與種子＝`seed`，帶表的事件全部掛上新 epoch 與新版本 token；`seed` 是 `null` 就不動模式。
- 寫入順序：先拿新桌 id 的整桌獨占再建桌（建桌途中失敗清掉半成品目錄）；桌內全部（角色、原卡、世界書、機制、逐字稿、控制檔、chat／character／script 層、旁檔）成功後才補跨桌層，桌內任何落檔失敗都算匯入失敗。失敗就退回已補的跨桌鍵並刪掉新桌：跨桌層以層的 rev 做 compare-and-set，補完之後那一層有任何寫入（含改回同值）就整層不退、保留別人的寫入；退不掉的鍵、刪不掉的桌一律連同原錯回報（`web_save_cleanup_incomplete`）。
- `card_storage` 交回前端，進新桌前寫進新桌的卡片 storage；寫不進去先問玩家重試，放棄就收掉新桌、這次匯入算失敗。匯入成功到前端確認之間，新桌的 `worlds/<id>/web-save-pending.json` 記著每層跨桌層補了哪些鍵與補完的 rev（建桌後先寫、每層寫入前先記）：寫好 storage 呼叫 `confirm_web_save_import` 刪掉它；放棄呼叫 `discard_web_save_import`，照記錄以同一套 rev compare-and-set 撤回跨桌層再刪桌，沒撤回的鍵同樣以 `web_save_cleanup_incomplete` 回報。
- `regex_allowed` 寫入桌層旗標（`state.json` 的 `regex_allowed`，D24）：false 時這桌的卡都不套卡內 regex 腳本（介面顯示腳本清空；桌面版本來就沒有送模前與玩家輸入的 regex）。一般匯卡與舊桌一律 true。旁檔 `worlds/<id>/web-save.json` 也留原值與 `opening_index`。
- 跨桌層補缺以頂層鍵為單位。
- 世界書路沒有角色：`character` 層不落地，非空時原樣存進旁檔的 `character_layer`（以存檔／旁檔仍在為準）。
- 不檢查容量：匯入整段寫完，滿了由下一句的換幕容量鎖擋，玩家照常換幕。
