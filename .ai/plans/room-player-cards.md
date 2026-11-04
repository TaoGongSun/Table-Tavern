# room-player-cards — 房間階段二：玩家匯入卡檔、編自己的卡（含秘密情報與換圖）

上層方案：[room-host-desktop-play.md](room-host-desktop-play.md)（§3 表格、§4.4、§4.5 是本案起點）、協定 [multiplayer-room-protocol.md](multiplayer-room-protocol.md)。分支建在 `room-host-desktop-play` 之上，階段一結案後 rebase 到 main 再開工。

**案別：大案**，核心估約 850 行（三包，§5）。碰到信任邊界（玩家上傳卡檔與圖、玩家送的卡片文字進 GM 提示詞、玩家觸發花 GM 主機額度的生圖），方案與驗收都送審。方案審第 1 輪（Sol）三條必改已併入（§3.1–§3.4）。

## 1 作者原話與裁決

- 「基本就是跟現在一樣只是做成連線版。」〔作者裁決 2026-11-03〕
- 「玩家要能編輯角色，要保留『不給其他玩家看的秘密情報』區塊，同時也能匯入角色卡。」〔作者裁決 2026-11-03〕
- 秘密情報＝GM 看得到但不主動揭露，跟 NPC 私有段同一個語意；編輯框寫明 GM 看得到，提示詞加一句不得主動揭露。〔作者裁決 2026-11-03〕
- 階段二在階段一結案後接著做。〔作者裁決 2026-11-03〕
- 玩家匯入的卡只收角色本身：狀態欄規則、卡片介面與正則、卡上世界書都不收。〔作者裁決 2026-11-03〕
- 單人桌也加：所有玩家卡（含單人桌）都有秘密情報欄，GM 看得到但不主動揭露。〔作者裁決 2026-11-03〕
- 手機能換角色圖：上傳圖＋AI 生圖；生圖照桌面版現有角色生圖路徑，花的是出額度者的額度。〔作者裁決 2026-11-03〕
  - v1 實際扣的是 GM 主機上設定的 AI 帳號（見 §3.4），方案把「出額度者」落實為 GM 主機。

## 2 成品樣子

- **手機選角畫面**多兩個入口，跟既有「選現成的卡」並列：
  - **匯入角色卡**：選 `.png`／`.json` 卡檔。上傳後房主電腦建出一張只有角色本身的卡，然後自動入座。失敗只顯示原因（格式不對、太大、解析失敗、GM 主機離線），不顯示檔案內容。
  - **建立角色**：原本的簡易自建（名字＋公開描述）換成完整編卡畫面，多一欄秘密情報。
  - 兩個入口都受房間設定「允許玩家自帶角色」（`allowCustomCards`）管，只有還沒入座時能用。已入座要換角，先離座再匯入或建立，跟 `seat.create` 一樣。
- **手機「編輯我的角色」**：入座後回合畫面多這顆鈕，打開編卡畫面。欄位照桌面版玩家卡的卡片編輯，依序是：
  - 角色圖：上傳、AI 生圖、移除。
  - 名字。
  - 圖示（emoji）。
  - 公開描述。
  - 秘密情報：說明寫「GM 看得到，其他玩家看不到」。

  其他行為：
  - 改名照桌面版，存檔前確認一次。
  - 存檔後名冊重送，大家看到新名字、公開描述與頭像。
  - 這顆鈕在桌面版的房間畫面不出現，電腦自己的座位照舊用 CastRail 的卡片編輯。
- **手機 AI 生圖**（§3.4）：
  - 生圖鈕旁常駐「會用 GM 主機「<主機名>」上設定的 AI 額度」，主機名是 `executor.label`。
  - 按下先確認一次（每次開編卡畫面只問第一次）。
  - 生出的圖先給預覽，玩家按「用這張」才換上。
- **桌面版玩家卡（單人桌與房間都一樣）**：
  - 卡片編輯多秘密情報欄，說明同手機，存檔不再清空 `private_md`。
  - 房間期間開朋友的卡照階段一：不顯示秘密段，存檔延續磁碟原值。
- **桌面版房間資訊列**多兩樣：
  - 「允許玩家用 AI 生圖」開關。
  - 最近生圖紀錄：「某某 生了一張角色圖」。
- **GM 提示詞**：
  - 單人桌的玩家卡、多人桌所有入座玩家的卡（含主玩家），都在 GM system 的「玩家角色」段放公開段＋秘密情報段。
  - 秘密段的標籤寫明「玩家自填的秘密情報：你知道，但不得主動揭露，也不能讓其他角色得知，除非劇情自然揭開」。

## 3 做法

### 3.1 上傳：先預留名額、再讀本體；伺服器只放記憶體、執行端拉取

照 §4.4 的形狀：HTTP 上傳到中繼點，處理端拉下來處理，手機不解析。跟 §4.4 不同的只有暫存位置（§6 第 1 條）。卡檔與角色圖走同一條通道，用 `kind` 分。

- **兩段式，讀本體前先預留**（Workers 由 DO 的 `fetch` 執行，Node 由 `http.ts` 執行，兩殼同形）：
  1. 收到 `POST /api/rooms/<碼>/cards?kind=card|image`，**先不讀本體**。
     - 沒有 `Content-Length` 回 411。
     - 超過上限回 413：卡檔 8 MiB、圖 2 MiB。
     - 有 `Origin` 而且跟自己的來源不同時回 403。
  2. 跑核心 `{ kind:"http", route:"upload_reserve", deviceId, uploadKind, length }`，依序驗：
     1. 成員驗證：`Authorization: Bearer <deviceId>`，跟匯出同一套，定長比較雜湊。
     2. 被封鎖回 403，房間已關回 410。
     3. 執行端不在線回 503。
     4. 依 `kind` 驗座位：
        - `card`：`allowCustomCards` 關著回 403，已入座回 409。
        - `image`：必須已入座，否則回 403。
     5. 限流與名額：每位成員同時一件、每小時 10 件；全房同時 3 件，而且預留總長度 ≤ 16 MiB。超過回 429。
     - 驗過就在 `ctx.live.uploads[uploadId]` 記 `{ memberId, kind, cardId?, length, state:"reserved", at }`，名額從這一刻起算。
  3. 名額拿到才讀本體，用串流讀。累計超過 `length` 或上限就中止，回 413 並釋放名額。
  4. 讀完跑 `{ route:"upload_body", uploadId, bytes }`，核心驗：
     - 長度要等於 `length`。
     - 簽章：`card` 是 PNG 簽章，或去掉 BOM、空白後是 `{`；`image` 只收 PNG。手機先把 JPEG、WebP 轉成 PNG（§3.5），入口只認一種格式。
     - 不合格回 415 並釋放名額；合格就把本體掛上、設到期時間（現在＋10 分鐘）。
     - 對執行端發 `job.card_import` 或 `job.card_image`：`{ jobId, memberId, cardId?, uploadId, size }`，HTTP 回 202 `{ uploadId }`。
- **取檔** `{ route:"card_fetch", executorToken, uploadId }`：
  - 驗執行端憑證，回本體，同時從記憶體丟掉本體。名額保留到結果回來。
  - 到期、已取走或 DO 已重啟都回 404。
- **名額一律收回**，以下任一情況發生就釋放，並對上傳者那條連線送 `vol.card_upload { uploadId, ok, cardId?, code? }`：
  - 結果回來。
  - 本體驗不過。
  - 讀本體中途斷掉。
  - 10 分鐘到期（順著鬧鐘清）。
  - 執行端斷線：全部待處理的直接判失敗。

  名額只在 `live`。DO 被逐出時一起消失，不會卡死。
- **結果**：執行端回 `job.result { jobId, ok, result | error }`。
  - 卡檔照 `cardResult` 做 `claim`。
  - 結果用 `tx.send` 只送上傳者，不寫事件。上傳者那時不在線就算了：入座是 `seat.claimed` 事件、換圖是名冊更新，重連都看得到。
  - `w.jobs` 只帶 id、memberId、cardId，不帶本體。
- **執行端**（Rust，新檔 `room/cards.rs`，`create_card` 一併從 `run.rs` 搬過來）：
  1. 用階段一已有的 reqwest 客戶端（`link.rs` 的 DELETE 那條）帶執行端憑證 GET 取檔。長度再驗一次，必須在上限內而且等於 `size`。
  2. **卡檔＝玩家模式建卡，不走 `import_character_file`**：
     - 為什麼不能走：`import_character_file` 一進來就先 `begin` 存原始來源（`files.rs` 的 `store_import_source`），再寫原卡檔、擴充與世界書（`card.rs` 的 `store_card_source`、`import_table_tavern_extension`、`character_book`）。玩家模式必須在寫入任何東西前就分流。
     - 改走新的純函式 `import::player_card_from_bytes(bytes, lang) -> { name, public_md, image: Option<png> }`，**不寫檔**：
       - 用同一支解析器 `parse_character`，含 PNG 尺寸像素上限與 512 MB 解碼配置上限。
       - 名字與公開段用 `public_markdown`，跟桌面版匯入同一份段標。
       - 顯示圖：PNG 卡走 `png_clean::salvage_stored_png`＋`strip_card_text`，再過 `validate_png_image(STORED_IMAGE_LIMITS)`；過不了就沒圖，卡照建。
       - 秘密段是空的。備用開場白、擴充、世界書、正則與介面都不讀。
     - 寫入照 `create_card`：
       - 在 `world_write_permit` 底下 `write_character`，有圖就 `save_character_image`。
       - 頭像照桌面版玩家卡現行做法。
       - 接著 `ensure_in_roster`、`table_changed`、重送名冊。
       - 圖寫失敗就收掉剛建的卡、回錯，不留半張。
     - **不存原卡檔與原始來源，也不記匯入收據**。這張卡等同玩家自建的卡（`seat.create` 也沒有收據）：
       - 不會出現在匯入紀錄，也沒有「復原匯入」。
       - 要移除就是一般刪卡（階段一已有「被認領的卡刪前確認」）。
       - 原卡檔沒存，介面與正則不可能生效（`interface.rs` 只從 `<id>.card.png`／`.import.json` 讀）。
  3. **角色圖**：卡要在 `player_card_ids` 而且等於 `cardId`。走 `check_character_image`＋`save_character_image`（只收 PNG、`STORED_IMAGE_LIMITS`），頭像照桌面版，重送名冊。
  4. `jobId` 去重照 `cards_made` 做法。
  5. 錯誤只回錯誤碼（照 `classify`），不回檔案內容與解析細節。

### 3.2 讀與存自己的卡：私設只過記憶體、座位雙重驗

- **私設不落地，連 `seat.create` 也一樣**：`seat.create` 照舊只帶名字與公開描述（≤ 1000 字），整筆照舊進 `w.jobs`，不加秘密段。
  - 手機「建立角色」填了秘密段時：`seat.create` 成功、入座後，客戶端自動接著送 `card.save { privateMd }`。
  - 這一步失敗（斷線、逾時、執行端離線）時，編卡畫面不關、秘密段原文保留，顯示「秘密情報還沒存上，請重試」。卡已建、已入座，只差秘密段。
- **`card.mine {}`**（P）：
  - 伺服器在 `live.cardReqs[reqId]` 記 `{ conn, id, memberId, cardId, at }`，對執行端發 `card.read { reqId, cardId }`。
  - 執行端驗那張卡存在、不是舞台卡、在 `state.player_card_ids`，回 `card.read.result { reqId, ok, card:{ name, avatar, publicMd, privateMd, image } }`；`image` 是給手機預覽的縮圖。
  - **回覆前伺服器再驗一次**：那位成員現在還坐在 `cardId` 上，不是就丟掉內容、回 `E_STATE_CONFLICT`。
  - 卡太大、執行端訊框（1 MiB）放不下時回 `E_TEXT_INVALID`，手機顯示「這張卡太長，請在電腦上改」。
- **`card.save { cardId, name?, avatar?, publicMd?, privateMd?, removeImage?, imageFromGallery? }`**（P，`cardId` 必須是自己座位上的卡）：
  - 只送改過的欄位，沒送的執行端照磁碟原值。
  - **訊框上限**：`validate.ts` 的 `LIMIT` 改成可以依 `t` 覆寫，**只有 `card.save` 放寬到 512 KiB**，其他照 16 KiB，不分片。
  - **伺服器驗**：
    - 名字照自建卡規則（1–40）。
    - 圖示 1–16 字元，不含空白與控制字元。
    - 兩段各 0–64,000 字元、NFC、`\r\n` 正規化；控制字元只允許 `\n` 與 `\t`。
    - `imageFromGallery` 格式照圖庫檔名規則。
    - 頻率每成員 2 秒一次。
  - **座位以伺服器收到請求那刻為準**：伺服器驗過才轉 `card.save { reqId, memberId, cardId, … }`。執行端照連線順序處理，這筆一定在之後的座位變動之前執行。
  - **執行端重驗**：
    - 長度與字元再驗一次，卡要在 `player_card_ids`。
    - 在 `world_write_permit` 底下讀現卡、只覆寫送來的欄位、`write_character`。
    - `removeImage` 走 `delete_character_image`；`imageFromGallery` 驗檔名在那張卡的圖庫裡，再走桌面版從圖庫套用的同一條路。
    - 名字走 `data::validate_single_line`。
    - 之後 `table_changed`、重送名冊、回 `card.save.result`。
  - 結果回來時伺服器同樣再驗座位，座位已變就只回 `ok`、不帶內容。
- **收尾**：`live.cardReqs` 的請求遇到以下情況就失敗，回對應錯誤碼，手機原文保留可重送：
  - 執行端斷線：回 `E_EXECUTOR_OFFLINE`。
  - 30 秒沒回：回 `E_EXECUTOR_TIMEOUT`，跟遠端控制同一個逾時。
  - DO 被逐出：請求一起消失，手機端 30 秒沒回就當逾時。
  - 成員座位變動：回 `E_STATE_CONFLICT`。
- **不變式**（寫成測試）：私設文字、上傳本體、生成圖不進 `RoomState`、不進 `events`、不進快照、不進匯出，也不寫日誌。

### 3.3 提示詞與桌面版 `CardEditor`

- **提示詞**（`transport/context.rs`）：「玩家角色」段放公開段＋秘密情報段，用 §2 的標籤，中英兩版。
  - 單人桌：現在只放公開段，加秘密段。
  - 多人桌：`party.present` 的每張卡（含主玩家）都進玩家段。現在其他入座玩家走「登場角色」迴圈、私設用 NPC 標籤，改成走玩家段，不再列在登場角色。
  - 秘密段是空的就不輸出標題。沒填的玩家，單人桌提示詞一字不變，快照與快取不受影響。
  - 角色線（`turns.rs` 的 `own.private`）不受影響，玩家卡沒有角色線。
  - 要確認一件事：`arrivals.rs` 用到 `private_md`。玩家卡中途登場（`dispatch.arrivals`）時，私設只能進 GM 上下文，不能進 `gen.part.end` 的公開正文。
- **`CardEditor`**：
  - `private_md: isPlayer ? "" : card.private_md` 改成延續卡上的值。
  - 秘密欄顯示條件從 `!isPlayer && !hidePrivate` 改成 `!hidePrivate`。玩家卡的說明，房間寫「GM 看得到，其他玩家看不到」，單人桌寫「GM 看得到，不會主動說破」。
  - 既有測試裡斷言玩家卡 `private_md: ""` 的要跟著改。

### 3.4 手機 AI 生圖：額度、告知、開關、紀錄都指向 GM 主機

- **誰的額度被扣**：生圖照桌面版 `generate_character_image`，用 GM 主機存的 `image_source` 與那台電腦上的金鑰或 CLI 登入。
  - 階段一的「出額度者」身分只是遠端控制權，交給手機後錢還是從 GM 主機的帳號出。
  - 所以本案把告知、確認、開關、紀錄全部指向 **GM 主機**（`executor.label`），不用 `payerId` 的暱稱。
- **開關在 GM 主機上**：桌面版房間資訊列放「允許玩家用 AI 生圖」，存在本機那桌的房間設定，預設開。
  - 執行端在 `status` 多報 `imageGen: boolean`，手機據此停用鈕並寫「GM 主機關閉了 AI 生圖」。
  - 真正擋的是執行端：關著就直接回 `E_FORBIDDEN`，不只靠伺服器與手機。
- **協定 `image.gen { cardId, name, description, extraPrompt, framing }`**（P，只能是自己座位上的卡）：
  - `name`、`description` 帶編卡畫面當下的草稿，跟桌面版一樣，沒存檔也能生。
  - `description` 手機端截到前 3000 字元（生圖提示用不到更長），`extraPrompt` 0–1000，`framing` 只收 `full`／`half`。
  - 整則照 16 KiB 上限，不放寬。
  - 限流：每成員同時一件、每小時 6 件，全房同時 2 件。名額在伺服器派出 `image.gen` 時占用，記在 `live.genReqs`；以下任一情況收回：成功、失敗、執行端離線、10 分鐘逾時、成員座位變動。
  - 執行端不在線，或 `status.imageGen` 是 false，伺服器直接回錯，不占名額。
- **執行端**：
  - 把 `commands/image.rs` 的 `generate_character_image` 本體抽成不吃 `AppHandle` 的函式，桌面命令與房間共用。
  - 執行端本機同時只跑一件；構圖照請求，來源固定用本機偏好。
  - 生成圖照桌面版進那張卡的圖庫，回圖庫檔名與縮圖預覽（要放得進 1 MiB 訊框）。
  - 錯誤照桌面版生圖的分類回碼（沒額度、來源不能生圖、被拒）。
  - 「用這張」就是 `card.save { imageFromGallery }`。
- **告知與紀錄**：
  - 手機生圖鈕旁常駐「會用 GM 主機「<executor.label>」上設定的 AI 額度」。
  - 每次開編卡畫面第一次按生圖，先確認：「這會用 GM 主機「<executor.label>」的 AI 額度生一張圖，要繼續嗎？」
  - 每次生圖（不論成敗）在桌面版房間資訊列的最近紀錄記一行「某某 生了一張角色圖」或「某某 生圖失敗」。紀錄只在 GM 主機本機，不上送、不進故事。

### 3.5 手機客戶端

新功能資料夾 `party/src/features/my-card/`：
- `CardSheet.tsx`：編卡畫面，建立與編輯共用；建立時「`seat.create` → `card.save` 秘密段」接力。
- `CardImage.tsx`：上傳、生圖、預覽、用這張。上傳前用 canvas 把 JPEG、WebP、PNG 一律重畫成 PNG，長邊上限照桌面版存圖上限，超過就縮。入口只收 PNG，伺服器與執行端照樣重驗。
- `card-upload.ts`：選檔、POST、等 `vol.card_upload`。

`SeatPicker` 接匯入與建立兩個入口。`RoomScreen` 入座後放「編輯我的角色」，`desktop` 時不放。`room-state`／`useRoom` 收新的回覆與 `vol.card_upload`。i18n 十語。

### 3.6 協定細節（包 1 定，包 2、3 照這份接）

- **上傳** `POST /api/rooms/<碼>/cards?kind=card|image`，`Authorization: Bearer <deviceId>`，本體是檔案原始位元組。
  - 成功 202 `{ uploadId }`（26 字 ULID）。失敗 `{ code, retryAfterMs? }`：400 `E_BAD_FRAME`（kind 不對）、401 `E_AUTH_FAILED`、403 `E_FORBIDDEN`／`E_BANNED`／`E_SEAT_REQUIRED`、409 `E_STATE_CONFLICT`、410 `E_ROOM_CLOSED`、411 `E_LENGTH_REQUIRED`、413 `E_TOO_LARGE`、415 `E_UNSUPPORTED_FORMAT`、429 `E_RATE_LIMITED`、503 `E_EXECUTOR_OFFLINE`。
  - 結果 `vol { type:"card_upload", uploadId, ok, cardId?, code? }` 送上傳者所有連線。
- **取檔** `GET /api/rooms/<碼>/cards/<uploadId>`，`Authorization: Bearer <executorToken>`：200 `application/octet-stream`；401 憑證錯、404 沒有或已取走。
- **工作** `job.card_import { jobId, kind:"card_import", memberId, uploadId, size }`、`job.card_image { jobId, kind:"card_image", memberId, cardId, uploadId, size }`；執行端回 `job.result { jobId, ok, result?:{ cardId }, error?:{ code } }`。
- **讀卡** 成員 `card.mine {}` → 執行端 `card.read { reqId, memberId, cardId }` → `card.read.result { reqId, ok, card?:{ name, avatar, publicMd, privateMd, image }, code? }`（`avatar` 是圖示 emoji，`image` 是 `data:image/png;base64` 縮圖或 null）→ 成員 `ok { card:{ cardId, …同上 } }`。
- **存卡** 成員 `card.save { cardId, …只帶改過的欄位 }` → 執行端 `card.save { reqId, memberId, cardId, …同欄位 }` → `card.save.result { reqId, ok, code? }`。
- **生圖** 成員 `image.gen { cardId, name, description, extraPrompt, framing }` → 執行端 `image.gen { reqId, memberId, cardId, …同欄位 }` → `image.gen.result { reqId, ok, file?, preview?, code? }`（`file` 是圖庫檔名、`preview` 是 PNG data URL）→ 成員 `ok { file, preview }`。
- 執行端回的 `code` 只認 `E_FORBIDDEN`、`E_STATE_CONFLICT`、`E_TEXT_INVALID`、`E_NAME_INVALID`、`E_TOO_LARGE`、`E_UNSUPPORTED_FORMAT`、`E_CARD_INVALID`、`E_GEN_QUOTA`、`E_GEN_UNSUPPORTED`、`E_GEN_REFUSED`，其餘一律當 `E_INTERNAL`。
- `status` 多 `imageGen?: boolean`，`ExecutorView` 多 `imageGen`；沒報（舊版執行端）當 false。
- 伺服器轉給成員時會再包一層，所以執行端的 `card.read.result` 整則要 ≤ 1000 KiB，放不下就回 `E_TEXT_INVALID`。
- 手機選檔後先自己比上限（卡檔 8 MiB、圖 2 MiB），超過就不送：宣告超過 16 MiB 的請求 Node 殼回完 413 就斷線，瀏覽器可能只看到連線錯誤。

### 3.7 包 2／3 介面（包 2 定的名稱）

- Tauri 指令 `room_image_gen({ enabled: boolean })`：資訊列「允許玩家用 AI 生圖」開關，存進這桌房間設定並馬上報伺服器；房間沒開回 `ROOM_NOT_ACTIVE`。
- `room_status` 回的 `Status` 多兩欄：`imageGen: boolean`（開關現值，預設 true）、`genLog: Array<{ at: number（毫秒）, name: string（卡名）, ok: boolean }>`（最近 20 筆，新的在後，只在記憶體）。資訊列照舊輪詢 `room_status` 讀。
- 生圖紀錄用卡名顯示（「<卡名> 生了一張角色圖」／「<卡名> 生圖失敗」），不用暱稱：執行端只確定知道卡名。
- 桌面版 `CardEditor` 的秘密情報欄、標籤與 `roomMode` 參數由包 2 做（i18n 兩個新鍵 `playerPrivateLabel`、`playerPrivateRoomLabel`）；包 3 不必碰。

## 4 建議（模型判斷·未裁決）

- 「允許玩家用 AI 生圖」開關做在 GM 主機上、預設開（§3.4）。理由：花的是那台電腦上的帳號，限流只擋得住暴衝，擋不住主機主人不想花。

## 5 分包

| 包 | 內容 | 核心估 |
|---|---|---|
| 1 伺服器 | `/cards` 路由與兩段式上傳（預留名額 → 串流讀本體 → 掛本體），兩殼同形；`card_fetch`、名額回收（結果、驗不過、中斷、到期、執行端斷線）與 `vol.card_upload`；`card.mine`／`card.save` 轉送、驗證、回覆前重驗座位、30 秒逾時；`image.gen` 轉送、名額占用與回收；`LIMIT` 依 `t` 覆寫；`status.imageGen` | 270 |
| 2 執行端 | `room/cards.rs`：取檔與重驗；`player_card_from_bytes`＋玩家模式建卡（不存原檔、不記收據、圖失敗收卡）；換圖；`card.read`／`card.save`（含圖庫套用與移除圖）；生圖抽共用函式＋房間處理＋本機開關與紀錄；`create_card` 搬入；提示詞玩家段含秘密段（單人＋多人）；桌面版 `CardEditor` | 300 |
| 3 手機＋桌面資訊列 | `my-card/` 編卡畫面、建立時秘密段接力、圖片區（canvas 轉 PNG、上傳、生圖確認、預覽）、上傳；`SeatPicker`、`RoomScreen` 接入口；桌面版資訊列生圖開關與紀錄；回覆處理；i18n 十語 | 280 |

合計約 850。包 1、包 2 可以並行（協定照本檔先定），包 3 等前兩包。D2 讓桌面版單人桌也動到（卡片編輯與單人提示詞），包 2 要跑單人桌回歸。

## 6 與階段一方案 §4.4／§4.5 的差異（送審）

方案審第 1 輪 Sol 認可第 1、3、4 條與「`card.save` 只送變更欄位」。

1. **上傳暫存放記憶體，不放 DO storage**：`RoomStore` 不動、兩殼不開表；協定 §9.1「不存卡片原檔」維持。代價是取走前 DO 被逐出或 Node 程序重啟要重傳，列已知限制。
2. **訊框上限**：只有 `card.save` 放寬到 512 KiB，`seat.create` 與 `image.gen` 維持 16 KiB；不分片。
3. **控制字元多允許 `\t`**：匯入卡常有 tab。
4. **匯入只在還沒入座時可用**，跟 `seat.create` 一樣。
5. **玩家匯入不走 `import_character_file`**：改走寫入前分流的純函式，不存原檔、不記收據（§3.1）。
6. **「出額度者」落實為 GM 主機**：v1 錢一律從那台電腦的帳號出（§3.4）。

## 7 驗收

- **自動**：每包 `npm run verify`、party test＋typecheck、`cargo test`。
  - 伺服器上傳：
    - 沒長度 411；超過上限 413，標頭與串流兩種都測。
    - 415（卡檔、非 PNG 的圖）。
    - 429（三種限流，加上預留總長度）；同時開 4 件上傳時，第 4 件在讀本體前就被拒。
    - 封鎖 403、已關 410、執行端離線 503。
    - 取檔取一次就 404、到期 404。
    - 名額在成功、驗不過、中斷、到期、執行端斷線五種情況都收回。
  - 伺服器讀寫與生圖：
    - `card.save`、`image.gen` 改別人的卡回 `E_FORBIDDEN`。
    - `card.mine` 回覆前換座：不帶內容、回 `E_STATE_CONFLICT`。
    - `image.gen` 名額在成功、失敗、離線、逾時、換座都收回。
    - 訊框依 `t` 的上限：只有 `card.save` 能到 512 KiB。
  - 不變式：上傳本體、秘密段字串、生成圖在 `state`、`events`、快照、匯出裡全文搜尋零命中；Node 殼的 SQLite 檔也要搜，包含 `seat.create` 後接 `card.save` 的流程。
  - 執行端：
    - 長度不符拒收。
    - 玩家模式建卡後，桌上沒有原卡檔、原始來源、收據、擴充、世界書條目，`read_card_interfaces` 讀不到這張卡；圖寫失敗不留卡。
    - `card.save` 不在座位上的卡拒收、只覆寫送來的欄位；圖庫檔名不在圖庫就拒收。
    - 本機生圖開關關著回 `E_FORBIDDEN`。
  - 提示詞：
    - 單人桌玩家卡有秘密段與禁止揭露句；**秘密段空白時，單人桌提示詞跟改前逐字不變**（快照比對）。
    - 多人桌所有入座玩家卡（含主玩家）都有秘密段。
- **測試通道實測**：本機 Node 殼＋測試通道桌面版開房，瀏覽器當兩支手機。
  1. 手機 A 匯入 `TestCards/` 的 PNG 卡與帶世界書、帶介面的 JSON 卡各一次：入座、名冊更新；桌上世界書、狀態欄規則、介面都沒多東西。
  2. 壞檔與超過 8 MiB 的檔只看到錯誤碼說明。
  3. A 編秘密段後存檔：A 重開編卡看得到；B 的畫面、名冊、故事都看不到。
  4. 桌面版開 A 的卡看不到秘密段，開自己座位的卡看得到、存檔不被清空。
  5. 跑一回合：GM 提示詞（除錯紀錄）含 A 的秘密段，旁白沒有主動講出來。
  6. 手機 B 用「建立角色」填秘密段入座：秘密段有存上；中途斷網時畫面保留原文、可重試。
  7. A 改名，名冊與 GM 點名用新名字。
  8. A 上傳 JPEG 圖（手機轉成 PNG），名冊頭像更新。
  9. A 按生圖：看得到「會用 GM 主機「…」上設定的 AI 額度」與確認框；用低階生圖跑一次、按「用這張」換上；桌面版資訊列出現紀錄。
  10. 桌面版關掉生圖開關，A 的生圖鈕停用。
  11. 單人桌：玩家卡填秘密段、玩一回合，GM 提示詞含秘密段。

## 8 已知限制

- 上傳取走前，DO 被逐出或 Node 程序重啟要重傳（§6 第 1 條）。
- 房主在 CastRail 與玩家在手機同時改同一張卡時，後存的贏。
- 卡大到執行端訊框放不下時，手機打不開編卡畫面，要在電腦上改。
- 秘密段會經過房間伺服器的記憶體（TLS 傳輸、不落地、不記日誌）；架伺服器的人原則上碰得到。
- 手機生圖只能用 GM 主機存的生圖來源，玩家不能選。
- 玩家匯入的卡不在匯入紀錄裡，不能「復原匯入」，只能刪卡。
- 繼承階段一：關房後用單人桌開這張桌，看得到所有人的秘密段。
- 執行端已建好匯入的卡、但 `job.result` 送達前斷線：伺服器在斷線時就判失敗，重送的結果只回 ok；那張卡留在名冊上沒人坐，玩家自己選它入座。

## 9 包 1 實作差異（主線送驗收）

1. `card.mine` 每人每秒一次（方案沒訂）。
2. 讀本體中途斷掉只收回名額、只回 HTTP，不送 `vol.card_upload`：那時手機還沒拿到 `uploadId`。
3. 新增錯誤碼 7 個（§3.6）；手機先對到通用錯誤，包 3 換成各自說明。
4. 取檔找不到回 404 `E_DISPATCH_UNKNOWN`。
5. 生圖名額跟讀卡、存卡記在同一張 `live.cardReqs`（`kind:"gen"`），不另開 `genReqs`。
6. 兩殼共用 `core/upload.ts` 收件（標頭 → 占名額 → 串流讀 → 交回核心），殼只轉換請求本體。被拒時先把宣告 ≤ 16 MiB 的本體讀掉丟棄再回：不讀就回，`wrangler dev` 實測連線會斷，瀏覽器也常收不到回覆。
7. 封鎖改在 `claim()` 擋：匯入與自建卡的結果晚到時，被封鎖的人不會入座。

