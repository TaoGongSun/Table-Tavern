# long-prompt-scene-hint：長桌撞命令列上限

立案見 [handoff](../handoffs/archive/long-prompt-scene-hint.md)。公開前必做〔作者裁決 2026-10-04〕。

## 1. 實測結果（2026-10-06）

### 1.1 命令列上限

| 平台 | 上限 | 怎麼算 | 超過時 | 證據 |
|---|---|---|---|---|
| macOS | argv＋env 合計 1,048,576 bytes（`getconf ARG_MAX`） | UTF-8 bytes：中文一字 3 bytes，純中文約 348K 字 | spawn 當場失敗 `Argument list too long (os error 7)`，CLI 沒有起來、沒送出 | python 二分搜尋三支真 CLI（claude 2.1.287／grok 1.0.46／agy 1.2.16，皆為原生 Mach-O）：ASCII 1,044,448 字、中文 348,149–348,150 字 |
| Windows | 整條命令列 32,767 個 UTF-16 單位（含程式路徑、旗標、引號跳脫） | 中文一字算 1 單位，約 32K 字；`"` 會被跳脫成 `\"` 算 2 | spawn 當場失敗 `The filename or extension is too long. (os error 206)`，`ErrorKind::InvalidFilename` | GitHub Actions windows-latest 用 `std::process::Command`（app 同一條路）二分搜尋：單一參數 32,715 字（ASCII、中文同數）、全是 `"` 時 16,357；stdin 管線送 8.4MB 中文完整收到。四家 CLI 在 Windows 都偵測 `.exe`，沒有 `.cmd` 包裝，8,191 那條不適用 |

現況各通道放在參數裡的東西（`src-tauri/src/cli/request.rs`）：

- claude：system 在 `--system-prompt`，正文已走 stdin。
- grok：system 在 `--system-prompt-override`，正文在 `-p`（生圖那條 system 併進 `-p`）。
- agy：system＋正文整包在 `-p`。
- codex：全走 stdin，不受影響。

結論：Windows 一張中文卡加世界書約 3 萬字就整輪失敗；macOS 要到約 35 萬中文字，那時 claude 的 200K context 早就先爆（見 1.3），所以 macOS 實務上只有 agy／grok 的大 context 模型撞得到。現況撞上時玩家看到的是 `AI_CALL_FAILED` 包住的 os error，前端落到 `errAiUnknown`「再試一次」——再試也不會好。

### 1.2 改走檔案／stdin 的可行性（每家都實測過）

- **claude**：`--system-prompt-file <path>`（`--help` 沒列，`--bare` 說明裡有提到）。3MB 中文 system 檔被接受、打到 API 才因模型不存在回 404；haiku 實送一次確認檔案內容真的坐在 system 層（回 PINEAPPLE）。
- **grok**：
  - 正文：`--prompt-file <path>`，3.6MB 被接受。
  - system：沒有 `--system-prompt-override` 的檔案版。改用 `--agent <檔案路徑>` 指向 agent profile（YAML frontmatter `promptMode: full`）。agent body 會被 grok 的模板引擎渲染（`${{ … }}`、`${% … %}`）而且頭尾空白會被修掉，所以 body 包成 `${% raw %}…${% endraw %}`，內文每個 `${%` 換成 `${% endraw %}${{ "${%" }}${% raw %}`。實測 session 落檔的 `system_prompt.txt` 與原文**逐 byte 相同**（含頭尾空行、縮排、假 frontmatter、`${{ }}`、`${% endraw %}` 破出嘗試、3MB 中文）。
  - **順帶發現 A（既有問題）**：grok 對 user 訊息超過約 100,000 bytes（ASCII 99K 原樣、101K 被截；中文 89.7K bytes 原樣、101.6K 被截）會把全文搬去 session 目錄的 `prompts/prompt_0.txt`，訊息只留約 33K 字節錄，叫模型用 `read_file` 去讀。我們聊天把工具全拆、`--max-turns 1`，模型讀不到——**長桌的 grok 現在就會靜默只看到節錄**（`-p` 與 `--prompt-file` 都一樣）。加 `--verbatim` 就不搬檔、也不再包 `<user_query>` 標籤，80K 字經 `-p`、30 萬字經 `--prompt-file` 都原樣送出。
  - **順帶發現 B（既有問題）**：全新的 GROK_HOME（沒有 `bundled/` 目錄）下，`--system-prompt-override` 被忽略，system 變回 grok 內建的 coding agent 提示（4300 字）；app 自己的 grok-home 目前有 `bundled/`、歷史 session 的 system 都正確。agent profile 那條在兩種 home 都正確。新玩家第一次用會不會落到沒有 `bundled/` 的狀態未查證。
  - **順帶發現 C（既有問題）**：grok 1.0.46 多了 `send_feedback` 工具，`GROK_CHAT_DISALLOWED_TOOLS` 沒列，`tool_definitions.json` 剩 1 支；走 agent profile 那條是 0 支。
- **agy**：不帶 `-p`、正文從 stdin 餵就是單發模式（`--output-format stream-json` 照常），新開與 `--conversation <id>` 續聊都實送確認過（gemini-3.6-flash-low）。
  - **順帶發現 D（既有問題，改傳遞方式解不掉）**：agy 對單則 user 訊息有約 192–200KB 的上限，超過只保留開頭、尾巴靜默丟掉，回傳仍是 SUCCESS。`-p` 與 stdin 完全一樣（同一份 400KB 標記檔兩條路都只看到第 1827 個標記、input_tokens 54,504／54,507；250KB 也是 54,503，與輸入大小無關）；中文版截在約 201KB。約 5MB 時直接回空回應、0 token、`warning: run ended with no output and no recorded error`。agy 的 system 是併在正文前面的，所以長桌被丟的是**尾巴——也就是本輪指示與最新對話**。
- **Windows 暫存檔**：stdin 已實測無上限；暫存檔是一般檔案 I/O，沒有長度問題。四家 CLI 在 Windows 實際讀 `--system-prompt-file`／`--agent`／`--prompt-file` 未實跑（CI 上沒有登入態）。

### 1.3 換傳遞方式之後的真正天花板

- claude：模型 context window。haiku 送 3.15MB 中文 system 回 `400 Prompt is too long · the request is ~1200740 tokens (limit 200000)`，`total_cost_usd: 0`（被擋的請求不計費）。中文約 1.1 token／字。
- agy：上面的 ~195KB 單則訊息上限，遠低於 gemini 的 context。
- grok（2026-10-07 grok-4.5 low 實測兩發）：
  - 130,000 bytes 正文＋`--verbatim`＋agent profile：session 的 user 訊息就是完整 130,000 字元、沒搬成附件，模型答出開頭與結尾的標記、照 profile 的 system 加了前綴。34,039 input tokens、$0.023。
  - 2.4MB 正文：伺服器上限 **500,000 tokens**，stdout 收尾 `{"type":"error","message":"Internal error: {… API error (status 400 Bad Request): … [input_too_large] The prompt is too long for this model's context window (544617 tokens > 500000 tokens) …}"}`、exit 1，不計費（session usage 全 0）。
  - 但 CLI 自己認的窗是 GROK_HOME `models_cache.json` 的 `context_window`（4.5–4.7 都是 256000）× `auto_compact_threshold_percent`（80）＝204,800。超過這個值，送出前 CLI 會先在本機自動壓縮（這發壓了兩次、花 54 秒）：共線遇到壓縮會丟線，單發可能被改動或延遲。所以 grok 的實際天花板是這個壓縮點，不是伺服器的 500k。
- codex：context window（本案不動）。

## 2. 範圍 2 施工（審查第 1 輪已併入，已施工）

### 2.1 暫存檔（審查第 1 輪已併入）

- 新檔 `src-tauri/src/cli/prompt_file.rs`：`PromptFile::create(dir, 檔名尾, 內容)` 寫入 `<config_root>/cli-prompts/<ulid>-<用途>`。`OpenOptions::create_new`，unix 建立當下就是 0600（不是寫完才 chmod），資料夾 0700；寫入失敗刪掉半成品再回錯。
- **刪檔時機**：`run_cli_cancellable` 改成「spawn 後的主體跑完，不論成功、錯誤（stdin 錯誤／逾時、stderr 致命錯、斷流、crash）、取消，一律先 `kill＋wait` 收屍再返回」。呼叫端在 `run_cli` 返回後才 drop `PromptFile`，那時子程序必定已退出，Windows 不會有程序還開著檔。
- **外層 future 被 drop**（中止在途呼叫時 select 輸掉的分支）：沒辦法 await 收屍，只靠 `kill_on_drop`。這時 `PromptFile::drop` 刪檔可能失敗（Windows 檔案仍開著），失敗就開一條背景執行緒每 500ms 重試、最多 10 秒；再不行留給下一條的清理。不宣稱即時刪除必成功。
- **殘檔清理**：每次建立前掃一次資料夾，只刪檔名符合本模組格式（26 字 ulid＋`-`）且超過 24 小時的檔；並發時檔案已被別人刪掉、或刪除失敗，都只記 log，不讓新請求失敗。
- 呼叫端持有 `PromptFile` 跨過整個 `run_cli` await。`LaneCall` 多帶 `prompt_dir`，單發路徑從 `config_root` 拿。

### 2.1b stdin 管線互等（審查第 1 輪）

現況 runner 先 `write_all` 完才開始讀 stdout／stderr；CLI 若先大量輸出、再讀 stdin，兩邊互等到 60 秒逾時。正文改走 stdin 後 agy 也會吃到。改法：寫 stdin 併進主讀迴圈的 `select!`（同一個 60 秒逾時、取消照樣優先），寫完即關 stdin；`BrokenPipe`（CLI 沒讀完就退出）不當錯誤，交給後面的退出碼／收尾判斷帶出 CLI 自己的錯誤。加反例測試：假 CLI 先吐 1MB stdout 再讀 2MB stdin。

### 2.2 各通道

| 通道 | 改成 | 參數產生器 |
|---|---|---|
| claude 單發／續聊／保溫／重構判官 | `--system-prompt-file <檔>`，正文照舊 stdin | `claude_args(model, system_file: &Path)`、`claude_session_args(model, system_file, session)` |
| grok 聊天單發、續聊開線 | `--agent <profile 檔>`（`promptMode: full`＋raw 包裝）＋`--prompt-file <檔>`＋`--verbatim` | `grok_args`／`grok_session_args` 改收 `Option<&Path>` system profile 與 `&Path` prompt 檔；profile 內容由新函式 `grok_agent_profile(system) -> String` 產生（純函式、可單測跳脫） |
| grok 續聊 `-r` | `--prompt-file`＋`--verbatim`，不帶 profile（system 凍在 session，維持現狀） | 同上 |
| grok 生圖 | system 照舊併進正文、走 `--prompt-file`；**不加** `--verbatim`、不帶 profile（保留原生 agent prompt 叫 image_gen 的路，正文短） | 同上 |
| agy 單發／續聊／生圖 | 拿掉 `-p <prompt>`，正文走 stdin | `agy_args(model, allow_tools)`、`agy_session_args(model, conversation_id)`；system 併正文改由呼叫端組好當 stdin |
| codex | 不動 | — |

- `run_cli` 本來就把 `stdin_data` 寫進管線，agy 只是從傳 `""` 改傳正文。
- 暫存檔寫失敗沿用既有 `CliWorkspaceFailed`（「無法準備 CLI 工作目錄」）人話，不新增文案。
- 測試通道 `harness::is_ai_probe` 認 `-p`；聊天參數不再帶 `-p` 不影響它（它只用在 `cli/install.rs` 的 `-p ok` 探測，那條很短、不改）。
- `--verbatim` 讓 grok 少了 `<user_query>` 包裝：既有續聊線的舊回合不變，新回合不包；前綴快取不受影響（變的只有新的尾段）。

### 2.3 順帶修

- 發現 C：`GROK_CHAT_DISALLOWED_TOOLS` 補 `send_feedback`（一行，測試的 26 改 27）。
- 發現 A、B 由 2.2 的 grok 改法一併解掉，不另做。

### 2.4 發現 D 的處置

不為截尾另做送出前擋下或專屬錯誤，交給範圍 3：提醒要提早——換幕本身也要把整幕送給後端，等到上限才提醒等於沒提醒〔作者裁決 2026-10-06〕。agy 的上限用 190,000 bytes（截尾門檻留餘裕）。世界書 4 萬字時 agy 常回空（system 本身就太大），提醒解不了，不另擋：照常顯示聊天容量提醒、保留一般空回覆錯誤，換幕容量鎖照 §3.1〔模型判斷·未裁決〕。

### 2.5 驗證（審查第 1 輪已併入）

- 單元測試：各參數產生器（檔案旗標、無 `-p`、`--verbatim` 只出現在文字通道）、`PromptFile`（create_new、0600、drop 刪檔、清理只動本模組檔、檔案已消失不失敗）。
- grok profile 跳脫：一次替換原文（`str::replace` 單趟，不會再替換插入的模板）。逐 byte 驗證用**真 grok CLI** 渲染（session 的 `system_prompt.txt`），案例含連續 `${%`、`${% endraw %}` 破出、CRLF、頭尾空白、`${{ }}`、`${#`、結尾 `$`、空字串；字串快照不算證明。以 `#[ignore]` 測試保存，手動跑。
- 正文組裝：假 CLI 實際讀檔／stdin 比對內容，不只看旗標——agy 新開＝system＋prompt、續聊只有 delta、補丁與降級重開 system 不漏不重複；grok 新開帶 profile、續聊不帶；claude 保溫與 refactor 判官讀 system 檔比對。
- stdin 互等反例（2.1b）。
- Windows CI（只在本分支觸發）：原生假 `.exe` 跑 `run_cli`——中文與空白路徑、長檔內容、並發不串檔、正常／取消／future drop 後檔案清理。三家真 CLI 在 Windows 讀檔仍標未驗證。
- `npm run verify`。
- 測試通道真 app 實送長桌：claude haiku；agy gemini flash low，總長控制在 190KB 截尾門檻以下；grok 正文自身 >100KB 驗 `--verbatim` 不搬檔（2026-10-07 直接打 CLI 通過，見 §1.3）。

## 3. 範圍 3：換幕容量提醒與鎖（施工計畫，Sol 第 1 輪意見已併入，待確認）

### 3.1 拍板

- 提醒門檻＝（該後端上限 − 換幕摘要那次呼叫所需空間）× 80%〔作者裁決 2026-10-06〕。
- 鎖送出只在真的送不出去時：再送一句就會讓換幕本身也送不出去的那一刻才鎖，讓換幕仍做得成；平常只提醒不擋〔作者裁決 2026-10-06〕。
- agy 照同一套，上限用截尾門檻（§2.4）〔作者裁決 2026-10-06〕。
- 提醒同時看聊天呼叫與換幕呼叫的長度；鎖仍只看換幕〔作者裁決 2026-10-06〕。
- 拿不到上限的後端不鎖；自訂 base_url 也不出容量提醒〔作者裁決 2026-10-06〕。
- 換幕本身送不出去時分段摘要再合併（§3.8）〔作者裁決 2026-10-06〕。
- 現有 `PlayView.tsx` 的 3 萬字軟提醒可接手擴充。

### 3.2 為什麼兩種呼叫都要看

換幕摘要（`transport::summary_messages`）只送角色側看得到的本幕紀錄＋約 200 字指示，不含 world.md／角色卡／世界書；聊天呼叫是 system＋本幕。claude 世界書大時聊天比換幕先撞上限；agy 續聊只送增量，但線一重開（換模、離開太久、任何對不上都丟線重建，`lanes/mod.rs` 開頭）就把 system＋整幕當一則送出，世界書 4 萬字時本幕約 7 萬 bytes 就靜默截尾。

### 3.3 符號與單位

每一種呼叫各自在**自己的單位**算一次，不混算、不重扣：

- `L`：該呼叫實際模型的**總 context**（輸入＋輸出＋思考共用）。「請求被接受」與「摘要能完整生成」是兩回事：Claude 新模型可能接受 input＋max_tokens 超過 context 的請求，但生成碰到總容量仍會停（`model_context_window_exceeded`），所以容量計算一律保留輸出空間，不設「純輸入上限」這一類。agy＝單則 body bytes（輸入截尾，已實測，`R=0`）。
- `F`：固定輸入——不隨本幕長大的部分。換幕呼叫＝摘要指示＋收尾句＋flatten 的標籤與換行＋CLI 自己加的固定開銷；聊天呼叫＝該路徑的 system／凍結內容＋回合尾段指示。
- `R`：完整生成所需的輸出空間，依供應商語義算一次、不重加：換幕摘要＝摘要輸出預留 4,096 token＋該模型的思考預算；Claude（API 與 CLI）思考已含在 max_tokens 內，`R＝min(實際送出的 max_tokens／CLI 回報的 maxOutputTokens, 4,096＋思考預算)`，不另加思考；其他供應商思考另計者才相加。聊天回覆同理，用該檔位回覆預留。agy `R=0`。
- `H`：本幕紀錄在該呼叫裡的量（換幕＝角色側事件行；聊天＝該路徑看得到的事件行＋動態條目）。
- 實作上 `F` 用「同一個組裝函式、事件清單換成空的」量出，`H＝整包 − F`，保證 F 只算一次。

判定：

| 狀態 | 條件 |
|---|---|
| 提醒（換幕） | `H_S ≥ 0.8 × (L_S − F_S − R_S)` |
| 提醒（聊天） | 任一聊天路徑 i：`H_i ≥ 0.8 × (L_i − F_i − R_i)` |
| 鎖 | `H_S ＋ 當次本句 ＋ G_reply > L_S − F_S − R_S`，且 §3.4 標為「可鎖」、且本模型有 §3.5 的校正值（agy bytes 是精確值，不需校正） |
| 已超過 | `H_S > L_S − F_S − R_S`：照常可換幕，走 §3.8 分段摘要 |

等號：提醒含等號、鎖不含（剛好塞滿仍送得出去）。

### 3.4 各後端上限、身分與失效

| 後端 | 單位／種類 | 來源 | 可鎖 |
|---|---|---|---|
| claude | token／總 context | ① 每次呼叫 result `modelUsage.<實際 model id>.contextWindow`（總 context；2026-10-06 haiku 實測回 200000）與同處的 `maxOutputTokens`（`R` 的上界）；② 錯誤 `prompt is too long … M` 的 M 當總 context 的佐證。存設定根 `context-windows.json`：`{transport, 別名, 實際 model id, 總 context, maxOutputTokens, 來源, 時間}`。下一次呼叫回報的實際 id 與記錄不同（CLI 更新換了別名對應）就整筆作廢。③ 都沒有時預設 200,000，只提醒 | ①② |
| codex | token／總 context | `models_cache.json` 中 slug＝實際模型的那筆，`context_window × effective_context_window_percent/100`；實際模型＝檔位覆寫，沒覆寫就讀 codex 自己 `config.toml` 的 `model`。讀的必須是 app 跑 codex 時用的那個 home（目前沿用使用者 `~/.codex`，施工時確認）。缺檔、損壞、找不到該 slug、預設模型解析不到 → 未知，只提醒不鎖 | 可（資料齊全時） |
| agy | bytes／單則輸入 | 常數 190,000（實測截在 192–201KB，§1.2 發現 D） | 可 |
| grok | token／總 context | app GROK_HOME 的 `models_cache.json` 中該模型的 `context_window × auto_compact_threshold_percent/100`（CLI 本機壓縮點，1.0.46 為 204,800；不是伺服器的 500k，見 §1.3）。模型＝檔位覆寫，沒覆寫就取模型目錄 grok 那組標 `(default)` 的。只反序列化 info 的這兩欄，api_key 不進任何結構；缺檔、解析失敗、找不到該模型或門檻、預設模型解析不到 → 預設 204,800，只提醒不鎖，原文不寫 log 也不進錯誤 | 可（資料齊全時） |
| API／OpenRouter | token／總 context | 目錄的 `context_length` 與 `top_provider.context_length` 取小、`top_provider.max_completion_tokens` 夾 `R`；路由到別的 provider 可能更小，所以只提醒不鎖 | 不鎖 |
| 穩定免費 | token | 用 `smart_free::plan_from_disk` 實際排出的候選（已扣排除與冷卻）中最大的 context，輸出預留同 `select::RESERVED_OUTPUT_TOKENS`；候選隨時在變，只提醒 | 不鎖 |
| API／自訂 base_url | — | 拿不到：不提醒、不鎖 | — |

錯誤抽到的數字一律當總 context（要扣 R）。分不出是哪個模型的不寫入。供應商回 `model_context_window_exceeded`（或其他「生成碰到容量上限而停」的停止原因）一律歸為不完整回覆（`AI_INCOMPLETE_RESPONSE`），換幕／重寫摘要時零提交。

### 3.5 長度怎麼量

新 Rust 模組 `src-tauri/src/scene_budget.rs`，Tauri 指令 `scene_budget(world_id, request_seq)` 放 `commands/scene.rs`。

- **一律量實際組裝**：各路徑抽出「組裝但不送出」的入口，與實送共用同一函式，不另寫一套近似。
  - 換幕：`summary_messages` → `cli::flatten_messages`；agy 再過最終 body 函式（把 `dispatch.rs` 單發與 `agy_session_body(.., None)` 的 `"{system}\n\n{prompt}"` 抽成同一個函式，三處共用）——量的是寫進 stdin 的那份 bytes。
  - 聊天：每一條實際會被用到的路徑都量——GM（`gm_materials`＋`assemble_gm_messages`／`gm_lane_turn`）、每個在場角色（`chat_with_character` 的組裝，含私設、角色側紀錄）；lane 後端量**重開全量**那份（system／凍結內容＋全量重建＋補丁），因為任何失敗都會降級成重開。各路徑用自己檔位的模型上限算比例，取最危險者。
- **token 換算**：`estimate_tokens` 原本只供診斷，不直接拿來判上限。改成：
  - 新的保守估計 `budget_tokens`（CJK、ASCII 字母數字、ASCII 符號與空白分開計），係數用夾具實測校準：中文散文、英文散文、中英混排、JSON／狀態區塊、符號密集（`{}[]"<>|` 等）各一份，送 haiku 一次取實報 input。
  - 校正值＝某次實送的「含快取完整 input usage（input＋cache_read＋cache_creation）」÷「那一次實際組裝的 `budget_tokens`」。送出當下就把估計值跟呼叫一起記下（同一筆帳本行加欄位），回報時配對，不拿事後重組的估計去配。
  - 校正值綁 `{transport, 實際 model id, 路徑種類（GM／角色／摘要）}`，不跨種類通用；模型 id 變了就作廢。不設上夾限（符號多的文本比率本來就可能大於 1.5，夾掉會低估）；下限 0.5 防壞資料。
  - 中文係數維持 1.45〔作者裁決 2026-10-06〕（重複字如「雨」實測約 2 token／字會低估，只影響沒校正前的提醒時機）。
  - 沒有校正值：用保守估計，只提醒不鎖。首次換幕前通常還沒有「摘要」種類的校正值，所以第一幕可能只提醒不鎖，由 §3.8 分段摘要兜底。
- `last_prompt_tokens` 只在 lane key（線種＋模型）、本幕、lane 世代都對得上時才採用，取它與組裝量的大者；對不上就忽略。

### 3.6 下一輪回覆增量 G_reply（預測，不是證明）

- `G_reply` 只用來決定「現在鎖不鎖」，是預測值，猜錯的後果由 §3.8 分段摘要接住，所以不需要也不宣稱精確。
- 一輪的定義：玩家這次動作會觸發的所有寫入——玩家句＋本輪所有回覆（多角色一輪要把每位角色的回覆加總，GM 推進／點名同理）。
- 預測（唯一算法，前後端共用）：`G_reply`＝本幕最近 10 個動作中，每個動作「全部回覆」（不含玩家句；多角色接力、GM 推進的所有回覆加總）的量取最大值 ×1.2，下限 1,500 token／4,500 bytes；沒有歷史時用下限。**按每次動作切段**〔作者裁決 2026-10-06〕：每次送出、GM 旁白、推進、點名各算一個動作——前端把動作的 `action_id` 蓋在這個動作寫下的每一則事件上（逐字稿 `action_id` 欄；GM 回合的登場紀錄由後端蓋），新段＝玩家句或不同的 `action_id`，沒 id 的事件併進目前這段；舊事件沒有 id 時退回以玩家句切段。連續推進沒有玩家句時預測是單一動作的最大量，不會併成一大段提早鎖。玩家句不進 `G_reply`，由判定式的「當次本句」單獨計入。
- **同一輪只 gate 一次（動作收據）**：現行是先 `append_player_event` 再 `chat_with_character`／GM，而且每則回覆各自一個 `turn_id`。所以前端為每個玩家動作（送出、請某某發言、旁白、推進、重試、卡片介面 `/send`）另產一個 `action_id`，傳給 `append_player_event` 與這個動作後續串起來的每一則回覆指令。
  - 第一個進後端的指令做 gate：`H_S＋當次本句＋G_reply > cap_S` 就拒（前端即時判定用同一式；無玩家句的動作本句＝0），通過就在記憶體發收據 `{world, action_id, scene, 設定世代}`。
  - 同一 `action_id` 的後續指令（玩家句已落檔後的回覆、多角色接力回覆）見到有效收據就不再 gate——H 已含玩家句，再加完整 G 會把剛通過的回覆錯擋。
  - 收據在換幕／退回／分岔、設定世代改變、或動作結束（前端回報或 10 分鐘逾時）時作廢；作廢後的回覆指令重新 gate。
  - 卡片介面 `/send` 在落玩家句**之前**就 gate，被擋時不留孤立玩家句；一般送出被擋在 append 前同理。

### 3.7 提醒、鎖、與並發

- **畫面**：輸入框上方那行。優先序：鎖＞容量提醒＞離開太久＞3 萬字。容量提醒與鎖帶「換幕」鈕，呼叫 `useSceneActions.advanceScene`（同工具列）。
- **鎖進動作入口，後端為準**：Rust 新增 `capacity_gate(world, action_id, extra_text)`，在既有桌級互斥（`world_write_permit`）內用最新幕與最新設定重算；`append_player_event`（含卡片介面 `/send` 落的玩家句）、`chat_with_character`、`gm_narrate`、GM 推進、重試都在寫入前呼叫，依 §3.6 的收據規則同一動作只算一次；滿了回 `UiMsg::SceneCapacityFull`（人話＋換幕鈕，同範圍 4 的呈現）。前端停用按鈕只是提示，不是防線。
- **容量回應防晚回**：`scene_budget` 回傳帶 `world_id`、`scene`、設定世代、`request_seq`；前端只收與目前桌／幕／世代相符且序號最新的；換桌、換幕、改設定時先清掉舊值，刷新期間不沿用舊值放行（反正後端 gate 會擋）。
- **前端時機**：進桌、每輪結束、換幕／退回／分岔後、設定改傳輸或模型後。鎖的即時判定（打字跨過門檻）在前端用回傳的 `H_S、cap_S、G_reply、unit、lockable` 算，送出時以後端 gate 為準。

### 3.8 分段摘要（換幕送不出去時）

`advance_scene` 與 `regenerate_scene_summary` 共用一個 `summarize_scene(events) -> Result<(title, summary)>`：

1. 單次放得下（`H_S ≤ cap_S`，或上限未知）→ 照舊一次呼叫。
每一層（分段、合併、再合併）都用**自己的** `F`／`R` 算可用容量 `cap = L − F − R`（總 context 時）；`cap ≤ 0` 或 `cap < 最小塊` 就明確早退回「摘要失敗」，不送。

2. 放不下 → 按角色側可見事件切塊，每塊各自 `≤ cap_分段`；單一事件自己就超過 → 按段落、再按句切；仍超過（沒有段落／句界的超長文字）就按 Unicode 字元邊界硬切（`char_indices`，不切開多 byte 字元）。
3. 每塊要求「≤ 1,500 字的中間摘要」，依序呼叫。1,500 字只是要求不是保證：回來後重新量，超過就要求重寫一次更短的版本，仍超過就以字元邊界截到上限並在該段標註「（節錄）」；任何中間摘要都要先量過才進合併。
4. 合併：所有中間摘要＋合併指示一次送出產生最終標題＋提要；合併輸入超過 `cap_合併` → 對中間摘要再做一層 2–4（遞迴），深度上限 3。
5. **縮塊**：任何分段或合併呼叫收到明確「太長」（範圍 4 的碼）——不論上限已知（估錯）或未知——該層塊大小減半重切重送；最小塊 4,000 token（agy 12,000 bytes），且不得大於該層 `cap`。
   - **次數上限統一 20 次**：所有分段、合併、重寫、縮塊重試都計入同一個計數，到 20 次就停。
   - **最終失敗**：次數耗盡、深度超過 3、最小塊仍收到「太長」、或 `cap` 早退，一律回 `UiMsg::SceneSummaryFailed`（人話「這一幕太長，這次整理沒有成功，原本的紀錄都還在」）；不掛 `AI_CONTEXT_TOO_LONG`、不顯示換幕鈕（按了只會重跑同一條路）。
6. **原子性**：全部呼叫成功才 `begin_next_scene`／`replace_scene_summary`；任一呼叫失敗、空回覆、截斷（`AI_INCOMPLETE_RESPONSE`）、取消，一律不寫任何東西、釋放鎖、回錯誤，原紀錄不動。
7. 保證範圍：本幕所有角色側可見原文都進過某一層摘要；不保證細節不丟（多一層摘要必然更壓縮）。呼叫次數視長度而定，不承諾固定次數。
8. 畫面：分段時換幕按鈕的進行中狀態顯示「整理中（第 k／n 段）」〔模型判斷·未裁決〕。

### 3.9 文案（zh-TW 草稿，十語系施工時補）〔模型判斷·未裁決〕

- 換幕呼叫觸發的提醒：「這一幕快到換幕整理的容量了，建議現在換幕。」鈕「現在換幕」
- 聊天呼叫觸發的提醒：「這一幕接近模型能一次讀的容量，預估再聊不久會送不出去，建議換幕。」鈕「現在換幕」
- 鎖：「預估再送一句會超過一次整理的容量，先換幕再繼續吧。」鈕「換幕」
- 換幕最終失敗：「這一幕太長，這次整理沒有成功，原本的紀錄都還在。」（不附換幕鈕）
- 3 萬字軟提醒文字不動。

### 3.10 測試與實測

- Rust 單元：
  - 判定邊界：`H = 0.8×cap` 提醒、差一不提醒；`H_S＋本句＋G_reply = cap` 不鎖、差一鎖；F 用空事件量、只扣一次（構造 F 很大的 system 驗證）。
  - 上限解析：claude result 抽 id、總 context 與 `maxOutputTokens`，別名對應換了作廢，錯誤的 M 只寫成總 context、不升格為輸入上限；codex 快取缺檔／損壞／缺 slug／預設模型未解析→未知不鎖；OpenRouter 取小；未知來源與無校正值永不鎖。
  - agy 量的 bytes 與單發、lane 開線實送 stdin 逐 byte 相同（假 CLI 收到的內容比對）。
  - 估計器夾具：中文、英文、混排、JSON、符號密集各自與實測比率在容差內；校正值換模型作廢、GM 比率不套到摘要。
  - `G_reply`：多角色一輪大回覆加總正確；沒有歷史用下限；短草稿、長草稿（單獨就超過 cap）、無玩家句動作（請某某發言、旁白、推進）三種邊界在前後端判定一致。
  - gate：滿時 `append_player_event`、`chat_with_character`、`gm_narrate`、推進、重試都拒且零寫入；卡片介面 `/send` 長 payload 在落玩家句前被擋、不留孤立玩家句。
  - 收據：append 剛好通過後同一 `action_id` 的回覆不被錯擋；多角色接力的第 2、3 則回覆不再 gate；換幕／改設定後收據作廢、回覆重新 gate；不同 `action_id` 不共用收據。
  - 分段摘要：剛好一塊、兩塊、單事件超限再切、無段落／句界的超長文字按字元邊界硬切（含 emoji、CJK、組合字元不切壞）、中間摘要超過 1,500 字被重寫或截斷後才進合併、合併再超限遞迴、每層用自己的 F／R、`cap ≤ 0` 或小於最小塊早退；已知上限估錯與未知上限遇太長錯誤都縮塊；所有重試共用 20 次計數；最小塊仍失敗／次數耗盡／深度超限回 `SceneSummaryFailed` 而非 `AI_CONTEXT_TOO_LONG`、錯誤列無換幕鈕；第二塊失敗／空回覆／截斷／取消後新幕與覆寫皆零寫入、鎖已釋放——`advance_scene`、`regenerate_scene_summary` 兩入口都驗（假傳輸，不打 AI）。
- 前端 vitest：提醒優先序與兩種文案；換幕鈕呼叫 advanceScene；鎖時動作鈕停用、工具列換幕不停用；打字跨門檻即鎖、刪字解鎖；換桌／換模後晚回的 `scene_budget` 被丟棄；刷新中送出交由後端 gate；十語系鍵齊。
- 測試通道（全機一把鎖，用前 ps、用完 quit；長逐字稿直接寫檔造）：
  1. claude haiku：估計器夾具校準（總數 ≤ 5）；確認 `contextWindow`、`maxOutputTokens` 與實際 id 記下；一次超長（被擋不計費）看範圍 4 錯誤與換幕鈕。
  2. agy gemini flash low：本幕約 150KB → 提醒；約 185KB＋打字 → 鎖、gate 擋送出；按換幕成功（1 次）；造 230KB 本幕驗分段摘要（約 3 次）。
  3. grok：一次超長呼叫取錯誤長相與上限（2026-10-07 直接打 CLI 做完，結果見 §1.3、§3.4、§4）。
- `npm run verify`。

## 4. 範圍 4：「太長了，請換幕」錯誤〔作者裁決 2026-10-06〕

- **只在結構化失敗裡認**，不在泛用錯誤全文掃關鍵字：
  - claude CLI：result 行 `is_error: true` 的 `result` 文字，或 runner 認定的 `API Error` 致命 stderr 行（`cli/runner.rs::api_error_kind`）；字樣 `prompt is too long`。
  - codex CLI：其 stream 裡的錯誤事件訊息；字樣 `context_length_exceeded`／`exceeds the context window`。
  - API 路：只在 HTTP 400／413 時解析 body 的 `error.code`／`error.message`（JSON）；`context_length_exceeded`、`maximum context length`；Gemini 要同一則 message 同時有 `input token count` 與 `exceeds the maximum`。5xx 包裝、2xx 正文一律不認。
  - grok CLI：streaming-json 的 `{"type":"error"}` 收尾行的 message；字樣 `prompt is too long`，另認錯誤碼 `input_too_large`（樣本見 §1.3，測試 `cli::stream` 的 grok 樣本）。
  - agy：不回錯誤（靜默截尾／空回應），不認。
- 認到就掛 `AI_CONTEXT_TOO_LONG:`（原文照附），並加進 `dispatch.rs::ai_call_failure` 的 `CODED` 白名單。順便把上限數字依 §3.4 寫入 `context-windows.json`。
- **前端**：`ai-error.ts` 的 `FAILURE_CODES` 加 `AI_CONTEXT_TOO_LONG → errContextTooLong`（只認開頭）；`ErrorNote` 收選用 `onAdvanceScene`，命中這個鍵或 `SceneCapacityFull` 時多一顆「換幕」鈕；只有聊天錯誤列傳入。
- **換幕本身收到**：`summarize_scene` 內部吃掉這個碼觸發 §3.8 的縮塊；最終失敗只回 `SceneSummaryFailed`，`advance_scene`／`regenerate_scene_summary` 永遠不把 `AI_CONTEXT_TOO_LONG` 或聊天用的「請換幕」文案丟給玩家。
- 文案（zh-TW 草稿）〔模型判斷·未裁決〕：「這一幕太長，模型一次讀不完。換幕整理成前情提要後就能繼續。」十語系。
- 測試：各家真實錯誤樣本→前綴；反例不認——否定句（`prompt is not too long`）、玩家或模型正文引用這句話、成功回覆正文含字樣、5xx body 包上游 400 字樣、Gemini 兩段字樣分在不同訊息；`ai-error.test.ts` 新碼與 body 不翻盤；ErrorNote 有鈕／無鈕。
