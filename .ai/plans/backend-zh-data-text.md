# backend-zh-data-text 規格

拍板〔作者裁決 2026-10-02〕：存語系無關的代碼，顯示與匯出時照當下語系翻譯；程式比對只認代碼；不做舊檔相容。

範圍調整〔作者裁決 2026-10-03〕：⑤ 是嵌在玩家可編輯內文裡的段標，不照 10-02 的「存代碼」做，改成寫入當下照介面語系產生；前端寫進逐字稿的固定文字（⑥⑦）併進本案。

## 盤點（行號以 main 8057017 為準）

| 組 | 位置 | 寫入點 | 程式比對點 | 顯示／匯出／送 AI |
|---|---|---|---|---|
| ① 前情提要 | `data/scene/lifecycle.rs:26-32` `format_scene_summary`（`【前情提要】`／`Previously:`，寫入當下 en/zh 二選一） | `begin_next_scene`、`replace_scene_summary`（:210） | 無（判斷「只有摘要」靠則數與 `forked`） | 畫面 `PlayView.tsx:162,169`、`ActReader.tsx:95`、卡片介面樓層 `card-chat-shim.ts:39`；匯出；下一幕提示詞 |
| ② 回歸／登場 | `data/scene/presence.rs:15` `（角色回歸）`、`transport/arrivals.rs:10` `（人物登場）`；連帶同一則事件的 `（角色私設）`（:107）、`私有設定：`（:110）、`公開設定：`（:118）（後三者原歸 E 類） | `commands/chat.rs` `record_person_arrivals`／`record_card_arrivals` | `presence.rs:25` `appeared_titles`（換幕結算 :78、`commands/scene.rs:164`、`arrivals.rs:27,80`）；`arrivals.rs:136-163` 私設略過／gm_only 只留首行／舊合併事件截斷；`lanes/mod.rs:327` 舊事件重開判斷 | 同①；送 AI 走 `turns.rs:38` `lane_event_line`、`assemble.rs:69,124`、換幕摘要、關鍵字掃描 `context.rs:13` |
| ③ 跑團紀錄匯出 | `data/scene/export.rs:87,94,122`（標題、`匯出時間：`、`## 場景 N`），已有 en/zh 二選一 | 匯出當下 | 無 | 匯出檔 |
| ④ 角色卡檔段標 | `data/character.rs:133-134,169` `## 公開`／`## 私有` | `serialize_character` | `parse_sections` | 不顯示（純存檔分隔線） |
| ⑤ 匯入內文段標 | `import/card.rs:14-18` `PUBLIC_SECTIONS`（`### 簡介` 等五個）、`:274` `### 備用開場白 N`；`data/worldbook.rs:586` 卡轉世界書 `## 私有` | 匯入卡、卡轉世界書 | `import/export.rs:70` `split_public_markdown` 依 `### 標題` 拆回 ST 欄位 | 寫進 public_md／private_md／條目內文：編輯器原樣顯示、原樣送 AI |
| ⑥ 前端系統事件 | `src/features/play/useChatController.ts:465` `stateUpdateHeader`（狀態更新）、`:507,511` `gmCallOn`（GM 請「X」發言） | 前端 `appendEvent` | 無 | 同① |
| ⑦ 玩家名退路 | `useChatController.ts:506,550` 沒有玩家名時 speaker_name 寫入翻好的 `t("playerLabel")` | 前端 `appendEvent` | 無 | 畫面名牌、卡片介面樓層與目前樓名稱 `card-shell-route.ts:103,109`、匯出 `**X**：`、提示詞 `X：` 行 |

再掃結果：其餘寫進逐字稿的是 AI 產出、`speaker_name: "GM"`、範例桌／一句話開桌的開場內文（`data/world.rs:207`、`genesis.rs:214`，屬內容），都不是固定標籤，不列入。

## 三種語系分開講

- **顯示語系**：畫面、卡片介面樓層、匯出檔、⑤ 寫入段標，跟介面語系走（十語系）。
- **marker 送 AI 語系**〔模型判斷·未裁決，交 Sol 審〕：照 `transport/context.rs` 既有慣例，`lang == "en"` 出英文，其餘出繁中。繁中字面：①②沿用現行字面，⑥ 用 zh-TW 字典現值（`狀態更新`、`GM 請「X」發言`）。
- **模型輸出語系**：維持 `messages.rs:7` `language_rule` 現行做法，本案不動。`messages.rs:29` 玩家退路名、language_rule 本來就隨介面語系變，本案不保證切語系時提示詞整包不變。

## 做法

### marker 契約（①②⑥）
- Rust：`TranscriptEvent` 加 `#[serde(default, skip_serializing_if = "Option::is_none")] pub marker: Option<EventMarker>`；`EventMarker` 為 `#[serde(tag = "type", rename_all = "snake_case")]`：`scene_summary`、`card_arrival { name }`、`person_arrival { title }`、`card_private { name }`、`state_update`、`gm_call { name }`，另加 `#[serde(other)] Unknown`（帶欄位的未知 type 也讀得進來，Sol 以 Serde 1.0.228 實測過）。JSON 形如 `{"type":"card_arrival","name":"X"}`。代碼落檔即持久契約，不得改名。
- TS：`backend-contracts.ts` 同形 discriminated union（`type` 為判別欄），外加 `{ type: string }` 退路型別。
- **未知 type 政策**：Rust 讀成 `Unknown`、TS 認不得的 type 都當「沒有標頭」——只顯示／送出本文，不參與任何比對。可能因新版資料、外部修改等原因出現；分岔複製等重寫整行的動作會把它寫成 `{"type":"unknown"}`，原欄位資訊流失，是已知限制。已知 type 缺必要欄位＝壞行，照現有壞行處理。
- `text` 只存本文：摘要、公開設定、條目全文、私設、`path：value` 各行；`gm_call` 本文為空。`gm_call.name` 空字串＝點名玩家且玩家沒名字，渲染時退回該語系的玩家稱呼。

### 組字與可見性（單一入口）
- 後端新檔 `data/scene/marker.rs`：
  - `marker_heading(marker, lang)`：十語系表（照 `leftover_title_suffix` 寫法，zh* 退繁中、未知退英文），只回第一行標頭，例如 `（角色回歸）〈X〉`。
  - `event_full_text(event, lang)`：標頭＋還原的段標與換行。`card_arrival` 本文非空白時為 `標頭\n公開設定：\n本文`，本文空白時只有標頭；`card_private` 為 `標頭\n私有設定：\n本文`；`person_arrival`、`scene_summary`、`state_update` 為 `標頭\n本文`；`gm_call` 只有標頭；沒有 marker 就是本文原樣。以繁中逐字測試鎖住與現行寫入字面一致。
- `prompt_text(event, lang, side) -> Option<String>`（`transport/arrivals.rs` 取代 `character_visible_text`），可見性只在這裡判斷：
  - GM 側：`event_full_text`。
  - 角色側：`card_private` 整則略過；其他帶 marker 的 gm_only 只出 `marker_heading`（不含公開／私有段標）；無 marker 或 `Unknown` 的 gm_only 照現行只留本文首行（不出空標頭）；其餘 `event_full_text`。GM 側的 `Unknown` 出本文。
- 送 AI 一律走同一條：`render_for_prompt(events, lang, side)` 產出渲染後的事件副本（`text` 換成 `prompt_text` 結果、`marker` 清成 `None`、略過的那則拿掉），下游只吃渲染文字：不得再遮罩、不得再組標頭，也不得拿副本做 lane 水位、雜湊或回覆對點。四條路都接上：換幕摘要（`summary_messages`）、共線 API（`assemble.rs`）、CLI lane（`lane_event_line` 逐則呼叫 `prompt_text`，加 `lang` 參數）、關鍵字掃描（GM 側與角色側都先遮罩再取全文，私設獨有的關鍵字不觸發）。
- 程式比對只認 `marker`：`appeared_titles` 取 `CardArrival.name`／`PersonArrival.title`。刪掉 `*_PREFIX` 常數、`PRIVATE_SECTION`、`is_legacy_card_arrival`、`character_events` 與 lanes `HistoryRedacted` 分支（不做舊檔相容：舊桌舊事件當一般訊息原樣顯示，舊合併回歸事件的私設不再對角色側遮蔽）。

### lanes 正典對點
- `events_fingerprint`（`lanes/mod.rs:180`）把序列化後的 `marker` 與 `gm_only` 一併雜湊，已送段改了這兩欄就 `HistoryEdited` 重開。水位、雜湊、expected_reply 一律用原始完整事件序列，不用渲染或遮罩後的序列。未送段照常增量送出最新內容，水位不變。
- `expected_reply` 對點（:335）：另外要求該事件 `marker.is_none() && !gm_only`（回覆事件不可能帶 marker）。不符就 `ReplyDiverged` 重開，避免只改了 marker／gm_only 卻落在雜湊水位外而被跳過。

### 前端（⑥⑦與顯示）
- `src/i18n/features/transcript-marker.ts` 十語系鍵；play feature 加 `eventDisplayText(event)`、`speakerDisplayName(event)`（player kind 且名字空白就退回 `t("playerLabel")`）。`PlayView`、`ActReader`、卡片介面樓層與目前樓名稱（`card-chat-shim.ts:39`、`card-shell-route.ts:55,103,109`）改用它們。
- `appendEvent`（`useChatController.ts:203`）：帶已知 marker 的事件允許空本文；沒有 marker 的空白事件照舊擋 `AI_EMPTY_RESPONSE`。`canRestore`（:188）把「有本文或有 marker」都算有內容。
- 寬鬆讀取保留 marker：`data/format/commit.rs:834` `ReadonlyLine` 加 `marker: Option<Value>`，後端原樣帶出、不驗；`open-world.ts:75` `looseTranscript` 轉事件時先過前端執行期形狀檢查 `parseMarker(value: unknown)`：須是物件、`type` 為字串；已知 type 缺 `name`／`title` 或不是字串就當畸形，畸形與未知 type 都當沒有 marker，只顯示本文。一般讀取路徑的顯示也走同一個 `parseMarker`。name/text/kind 的寬鬆規則不變。
- ⑦：沒有玩家名時 speaker_name 存空字串；提示詞退回 `player_fallback_name(lang)`，匯出退回匯出語系的玩家稱呼。

### ③ 匯出
標題、匯出時間、場景標頭擴成十語系，系統事件用 `event_full_text(event, 匯出語系)`，玩家空名照⑦退路；fr 標點前用 U+00A0。

### ④ 角色卡檔
- 分隔線改成獨占一行的 `<!-- tt:public -->`／`<!-- tt:private -->`。
- 跳脫（逐行）：設一行前導反斜線數為 k、其後剩餘為 r。寫入時，r 以 `<!-- tt:` 開頭的行輸出成 k+1 個反斜線＋r，其餘原樣；讀取時，k≥1 且 r 以 `<!-- tt:` 開頭的行還原成 k−1 個反斜線＋r，k=0 且整行等於分隔線才算分隔線。程式碼圍欄裡也照跳，不必辨識圍欄。
- 讀取嚴格：必須恰好 public、private 各一行，且依序出現，否則整張卡解析失敗，不默默填空或覆蓋。CRLF 照現行容忍 `\r`。
- 解析失敗的範圍：清單略過、玩家上下文視為無卡；直接 id 操作仍回錯誤。檔案留在磁碟不動：
  - `list_characters`（`character.rs:227`）改成 frontmatter＋段落都解析，失敗照既有 `略過無法解析的角色卡` 略過。`load_active_cards`、`load_hidden_cards`、換幕結算、排序都只讀清單裡的卡，不會整批失敗。
  - `read_player_card` 只把段落解析失敗轉 `Ok(None)`（與「玩家卡檔不存在」同待遇），實際 I/O 錯誤照傳播；前端 `useCharacterController.ts:120` 直讀失敗本來就當沒有玩家卡。
  - 直接 id 路徑（`commands/chat.rs:79`、`commands/character.rs:23`、`CardEditor.tsx:117`、`data/character.rs:351,364`、`worldbook.rs:579`、`import/export.rs:19`）解析失敗回錯誤，不得把解析失敗轉成空卡後繼續寫；`chat.rs:79` 失敗時不送 AI。`receipts.rs:745` 復原匯入讀名失敗仍刪除，可接受。
  - 已知限制（不救）：舊 `state.player_card_id` 殘留會讓 `refactor/apply.rs:43`、`worldbook.rs:529` 認為已有玩家卡，擋下建立／升格。
  - `character_file_parses`（格式遷移／重置的 `world_reads_fully`，`format/commit.rs:310`）維持只驗 frontmatter，舊卡不讓整桌遷移失敗。開桌的「需修復」判斷不讀卡片內容，不受影響。
  - 不選「整桌走需修復」：開桌要多掃全部卡檔，且修復頁也救不回舊格式，結果只是整桌打不開。
- 舊卡檔（`## 公開` 格式）＝解析失敗（不做相容），結果見下方驗收矩陣。

### ⑤ 匯入內文段標〔作者裁決 2026-10-03〕
- 匯入卡（含世界書卡轉人設）與卡轉世界書（`character_to_worldbook_entry`），寫入當下照介面語系產生段標（十語系：五個欄位、`備用開場白 N`、私有段標），之後就是玩家內文、不隨語系變。介面語系從指令層 `ui_language(&config)` 一路傳到 `import::card`、`data::worldbook`，`refactor/reset.rs` 重放匯入時用它既有的 `lang`。
- 匯出拆欄（`split_public_markdown`）解析規則：
  - 只在 Markdown 圍欄外辨識段標。圍欄照 CommonMark：開頭行縮排 ≤3 空白、≥3 個同一種符號（``` 或 ~~~，反引號圍欄的 info 不得含反引號）；關閉行縮排 ≤3 空白、同一種符號、長度 ≥ 開啟長度、後面只有空白；未閉合的圍欄延伸到結尾。
  - 段標＝整行（去掉行尾空白）完全等於 `### <任一語系的欄位名>`，逐段獨立辨識十語系，混語系（含玩家手動編輯）也拆得對。
  - 已知歧義：內文自然出現、與段標同字的行（例如英文描述裡的 `### Scene`）仍會被當成段標切欄。只承諾 App 自己寫的段標拆得回去，不承諾無損還原。
- 語系無關的結構分隔符：私有條目 `- **關鍵字**：內容` 的 `、`（`import/card.rs:255`）與 `：`（:256）、反向比對 `import/export.rs:100,103`，寫入與讀取兩端都固定不翻，列為格式契約。

## 分包
1. ①②⑥⑦：marker 契約、組字與可見性、四條送 AI 路（含摘要）、lanes 對點、前端寫入／顯示／寬鬆讀取。
2. ③：匯出十語系。
3. ④⑤：卡檔分隔線，以及匯入與卡轉世界書的介面語系一路傳到底、匯出拆欄規則。

## 測試清單
- marker：各 type 序列化往返，JSON 逐字；缺 `marker` 的事件照讀；帶欄位的未知 type 讀成 Unknown、只出本文；Unknown＋gm_only 角色側只留本文首行；前端 `parseMarker` 擋未知、缺欄位、欄位非字串。
- 摘要／回歸／登場寫入的 `text` 不含任何語系字面；`appeared_titles` 只認 marker（text 偽造 `（角色回歸）〈X〉` 不算登場）；換幕結算照舊。
- `event_full_text` 繁中逐字：有公開本文、公開本文空白（不多生段標、不多換行）、私設、登場、摘要、狀態更新、點名（含空名）。
- 可見性：`card_private` 略過、帶 marker 的 gm_only 只剩標頭、無 marker 的 gm_only 留首行；GM 側全文。私設獨有的關鍵字在角色側不觸發。四條送 AI 路各一則案例，確認標頭只出現一次。
- 提示詞模板：用指定 fixture（marker 事件＋⑦ 空名玩家）驗 zh-TW、ja、ru 出繁中標頭、en 出英文標頭；不拿舊存檔比逐字。
- lanes：已送段只改 marker 或只改 gm_only 會重開；expected_reply 那則帶 marker 或 gm_only 會重開；未送段改動照常增量送出最新內容、水位不變。
- ⑥⑦ 前端：純 marker 點名事件落檔→收回→復原全程可用；沒有 marker 的空白事件仍擋；寬鬆讀取帶回 marker、點名不是空行；卡片介面樓層與目前樓名稱的玩家名退路；切語系標頭跟著變；check-i18n 十語系鍵齊。
- ③：十語系標頭逐字、fr NBSP、系統事件標頭照匯出語系。
- ④：往返；內文行前有 0／1／多個反斜線的 `<!-- tt:` 行逐字還原；圍欄內同名註解、CRLF；重複／缺失／錯序分隔線都解析失敗。
- ⑤：十語系各自匯入→匯出拆回原欄位；五欄混語系段標拆回；圍欄規則（同符號配對、關閉長度、≤3 空白縮排、4 空白不算圍欄、未閉合延伸到結尾）；自然同名段標照規則切；卡轉世界書私有段標照介面語系。

## 驗收矩陣

| 情境 | 格式遷移／重置驗證 | 開桌 | GM 回合 | 角色聊天 |
|---|---|---|---|---|
| 舊 NPC 卡 | 通過（只驗 frontmatter） | 能開；側欄不列這張卡 | 不載入、不參與回歸偵測 | 點不到（不在清單） |
| 舊玩家卡 | 通過 | 能開；玩家卡區空白 | 當作沒有玩家卡，玩家名走退路 | 同 GM |
| 新舊混合 | 通過 | 新卡照常、舊卡不列 | 只載入新卡 | 只能點新卡 |
| 混語系五欄 | — | — | — | 匯出拆回原欄位 |
| 過期 id（舊卡）直接聊天／編輯／匯出／轉世界書 | — | — | 回錯誤、不送 AI | 回錯誤、不寫檔 |
| 舊玩家 id 殘留 | — | — | 擋下重構建立玩家卡、世界書升格玩家卡（已知限制） | — |
| 未知 marker type | — | 可讀、只顯示本文 | 只送本文 | 只送本文（gm_only 只留本文首行） |
| 畸形 marker（已知 type 缺必要欄位） | — | 正常讀取＝壞行，照現有壞行處理；寬鬆讀取由前端降級成只顯示本文 | 照壞行處理 | 照壞行處理 |

未知／畸形 marker 且本文為空時，不保證畫面不出空行。
