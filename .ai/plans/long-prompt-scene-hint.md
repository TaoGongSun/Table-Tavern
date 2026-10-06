# long-prompt-scene-hint：長桌撞命令列上限

立案見 [handoff](../handoffs/long-prompt-scene-hint.md)。公開前必做〔作者裁決 2026-10-04〕。

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
- grok：context window，錯誤長相待 22:00 額度恢復後實測（帳號目前 `402 Grok Build usage balance exhausted`）。
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

### 2.4 不在範圍 2 處理（待決定）

- 發現 D（agy ~195KB 截尾）：改傳遞方式解不掉。可選：
  1. 送出前在 Rust 端量 bytes，超過約 190,000 就直接失敗、走人話錯誤（「這一幕太長，換幕後再繼續」），不送一個會被截尾的請求——缺點是要新增錯誤碼與十語系文案。
  2. 只靠範圍 3 的提醒，agy 的門檻設在 190KB——提醒被忽略時仍會靜默截尾。
  3. 不處理。
  建議 1＋2 都做，但牽涉玩家看到的文案，列入待使用者決定。

### 2.5 驗證（審查第 1 輪已併入）

- 單元測試：各參數產生器（檔案旗標、無 `-p`、`--verbatim` 只出現在文字通道）、`PromptFile`（create_new、0600、drop 刪檔、清理只動本模組檔、檔案已消失不失敗）。
- grok profile 跳脫：一次替換原文（`str::replace` 單趟，不會再替換插入的模板）。逐 byte 驗證用**真 grok CLI** 渲染（session 的 `system_prompt.txt`），案例含連續 `${%`、`${% endraw %}` 破出、CRLF、頭尾空白、`${{ }}`、`${#`、結尾 `$`、空字串；字串快照不算證明。以 `#[ignore]` 測試保存，手動跑。
- 正文組裝：假 CLI 實際讀檔／stdin 比對內容，不只看旗標——agy 新開＝system＋prompt、續聊只有 delta、補丁與降級重開 system 不漏不重複；grok 新開帶 profile、續聊不帶；claude 保溫與 refactor 判官讀 system 檔比對。
- stdin 互等反例（2.1b）。
- Windows CI（只在本分支觸發）：原生假 `.exe` 跑 `run_cli`——中文與空白路徑、長檔內容、並發不串檔、正常／取消／future drop 後檔案清理。三家真 CLI 在 Windows 讀檔仍標未驗證。
- `npm run verify`。
- 測試通道真 app 實送長桌：claude haiku；agy gemini flash low，總長控制在 190KB 截尾門檻以下；grok 正文自身 >100KB（驗 `--verbatim` 不搬檔），等 2026-10-06 22:00 額度恢復後做。

## 3. 範圍 3 設計草案（上限前提醒換幕）——待使用者決定，不施工

現況：`PlayView.tsx` 已有軟提醒「紀錄很長，模型可能顧不上前面，建議換幕壓縮」，門檻是本幕事件文字合計 3 萬字（不含 system），只是建議。

草案：加一層硬提醒「再不換幕就送不出去了」。

- **門檻**：下一次呼叫的預估大小 ≥ 該通道上限的 80%。預估＝本桌 system＋本幕紀錄＋換幕摘要指示（換幕那次要把整幕送 AI，所以要拿換幕呼叫的大小來比，不是聊天呼叫）。上限依通道：claude／codex／grok／API 用模型 context window（token，優先用上一輪帳本實報的 input tokens 外推）；agy 用 190KB（發現 D）。
- **位置選項**（擇一）：
  - A. 沿用軟提醒的位置（輸入框上方那行），換成較醒目的樣式＋直接附「換幕」按鈕。
  - B. 聊天室裡插一行系統提示（同換模提示行：在玩家句與回覆之間、不進逐字稿），附「換幕」按鈕。
  - C. A＋到達 100% 時鎖住送出鍵，只剩「換幕」可按。
- **文案**（zh-TW 草稿，十語系施工時補）：「這一幕快塞不下了，再聊幾句就沒辦法換幕整理。現在換幕？」〔模型判斷·未裁決〕
- **待決定**：門檻比例（80%？）、位置 A／B／C、到 100% 要不要擋送出、agy 2.4 選項。

## 4. 範圍 4

改傳遞方式後不會再有 os error 7／206。真的爆 context 時：claude 回 `Prompt is too long`（400），目前前端認不出來會落到 `errAiUnknown`。是否新增「太長，請換幕」類別牽涉文案，與範圍 3 一起待決定。
