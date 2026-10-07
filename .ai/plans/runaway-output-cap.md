# runaway-output-cap 計畫

模型持續吐字（空白／亂碼／無限重複）時，單輪要能自己停下。現況只有「停止吐字」的逾時，持續吐字攔不下。

## 1. 現況查證

### 1.1 現有的限制（全部只管「沒輸出」，不管「輸出太多」）

| 路徑 | 讀法 | 現有限制 | 出處 |
|---|---|---|---|
| API（OpenRouter Chat Completions） | `bytes_stream` → `SseParser` → `extract_delta` 逐塊 `on_delta` | 首個合格進展 300 秒、之後 120 秒無進展＝`AI_STREAM_STALLED`；請求不帶 `max_tokens` | `transport/stall.rs:11-14,53-68`；`transport/client.rs:285-295,540-578` |
| API 智慧免費 | 同上，單模型 | 同上，失敗回 `ApiFailure::stalled` | `client.rs:691-735,764-766` |
| API Responses 模式 | 同上，`responses_progress` | 同上 | `transport/responses.rs:281-318,348-349` |
| CLI 四家（`run_cli_cancellable` 共用讀迴圈） | stdout 逐行 `next_line`，`parse` 出 `CliLine::Delta` 才累加 | 任何一行 stdout／stderr 都重置 120 秒（`sleep` 每圈重建）；stdin 60 秒；程序亡後 800ms | `cli/runner.rs:233-237,243-311,445-449` |
| claude／grok／agy 聊天與 GM | 走 lane 續聊，同一讀迴圈 | 同上 | `lanes/mod.rs:1015-1055` |
| codex 聊天 | 單發 `run_cli`；只在 `item.completed` 一次給整則訊息，沒有逐字增量 | 同上 | `transport/dispatch.rs:501-534`；`cli/stream.rs:299-315` |

- 四家 CLI 都沒有帶輸出長度旗標（`cli/request.rs` 全檔無 max output 參數）。
- grok 的 `text` 事件每個增量一行（`cli/stream.rs:397-406`），這次事故的空白亂碼每行都在重置 120 秒，所以 6 分鐘、約 295KB 都攔不住。
- `lines()` 單行長度沒有上限：CLI 吐出一條不換行的超長行時會整條吃進記憶體（`runner.rs:223,227`，stdout 與 stderr 都是）。本案順手處理。
- 收尾 fallback：整輪沒收到任何增量時，改用 `Done.text` 全文（`runner.rs:503-511`）。
- 思考增量不算正文：
  - claude 的思考解析成 `CliLine::Thinking`（`cli/stream.rs:25`）；
  - grok 的 `thought` 解析成 `Other`（`cli/stream.rs:908`）；
  - API 的 `reasoning`／`reasoning_content` 會讓停滯計時續命，但 `extract_delta` 只取 `content`（`stall.rs:94-109`）。

### 1.2 中止路徑現況

- **玩家按停止**：CLI 讀迴圈內收到取消就殺程序，回 `CliFinish::Aborted(半截)`（`runner.rs:249-252,460-463`）。lane 走 `settle_abort`（`lanes/mod.rs:736-790,1073-1088`）：
  - grok：整條撤線。
  - claude：照抹私設。
  - agy：不抹也不刪。
  - claude 和 agy 都留著呼叫前寫下的 `pending_rewrite`（`lanes/mod.rs:984`），下一輪因此重開全量（`lanes/mod.rs:553-556`）。API 路徑由 `take_abort_or_finish` 跟取消賽跑，丟掉 future 後回 delta 緩衝（`commands/chat.rs:41-65`）。
- **CLI 斷流**：殺程序，回 `CliReplyError{CliStalled}` 錯誤（`runner.rs:306-310,464-470`）。
- **lane 續聊遇到任何錯誤**：丟線、**重開全量再試一次**（`lanes/mod.rs:1201-1216`）。上限如果走錯誤路徑，續聊那輪會自動再燒一次全量。開線那次出錯：grok 撤線、claude 刪線，回錯（`lanes/mod.rs:1217-1234`）。
- **智慧免費**：已吐出正文就不重送（`smart_free/call.rs:150-157`）。只有 Network／Timeout 階段與 408／502–504 這類失敗才算模型層級、計入換模；串流階段的其他失敗歸類為 Other（`smart_free/failover.rs:70-97`）。
- **卡重構**：先試 claude session 的 `resume_stage`，失敗由 `result.ok()` 吞掉，改走全量單發（`commands/refactor.rs:160-200`）。這是另一條自動重派送。
- **帳本**：
  - API：中斷前收到的 usage 照記，再判成敗（`client.rs:586-604`）；usage 通常在最後一塊才到，所以停滯中斷時多半沒有。
  - CLI：解析到 usage 行當下就落帳，早於正文解析（`runner.rs:338-443`）；殺掉程序就沒有收尾的 usage 行。

### 1.3 前端呈現現況

- **錯誤**：invoke throw，半截從畫面消失、不落檔（`src/features/play/useChatController.ts:508-518,612-617`），彈 `TurnFailedDialog`（`src/features/play/TurnFailedDialog.tsx:13`）。玩家自己送出的那句會收回放回輸入框（`useChatController.ts:853-887`）。前綴碼由 `src/shared/ui/ai-error.ts:18-28` 對應到 10 語系主字典的鍵，例如 `errStreamStalled`。
- **中止**：`aborted: true`，半截非空就照常落成一則並標 `truncated`（`useChatController.ts:567-580`，GM 約 668-680）。GM 旁白中止會剝掉狀態欄、不寫狀態（`commands/chat.rs:281-295,382-385`）。
- 聊天回合沒有重試鈕。

### 1.4 一般回合實際輸出量（測試 root 抽樣，24 份 prompt-cache.jsonl、所有測試桌 transcript）

| 類別 | 中位數 | 最大 |
|---|---|---|
| 角色台詞落檔字數 | 475 字 | 844 字 |
| GM 旁白原文（含狀態欄）字數 | 3,331 字 | 4,999 字 |
| 續聊 output_tokens（含思考），claude GM | 3,694 | 12,921 |
| 續聊 output_tokens，grok chars／gm | 569／465 | 1,302／1,213 |
| 單發（卡重構、開桌等）output_tokens，claude | 1,327 | 15,172 |

事故那輪約 295KB，比一般回合的最大值高兩個數量級。

## 2. 做法

### 2.1 偵測規則（新模組 `src-tauri/src/transport/runaway.rs`，與 `stall.rs` 並列）

`RunawayGuard::new(policy)`，`push(delta) -> Option<RunawayReason>`。每次增量都逐字元更新狀態，所以同一段文字不論怎麼分塊、合併，判定結果都相同。

- **字元與空白**：
  - 字元一律按 Unicode scalar（`chars()`）計。
  - 空白按 `char::is_whitespace`（Unicode White_Space，含換行、tab、全形空白 U+3000）。
- **字數上限（A2）**：本輪正文累計超過 30,000 字元就觸發。單一增量本身超過也觸發，例如 codex 一次給整則。
- **連續空白**：連續空白達到 2,000 字元就觸發；遇到非空白字元就歸零。
- **空白比例**：維護最近 4,000 字元的滑動窗口（環形計數）。窗口要滿 4,000 字元之後，空白達 90% 以上才觸發；未滿時不判，開頭幾個換行不會誤殺。每吃進一個字元都檢查一次，不是只看每塊的結尾。
- **能力範圍**：這套規則只攔「空白型退化」和「總長度」，不宣稱能辨識一般的非空白亂碼。非空白的失控只有字數上限攔得到。
- **觸發理由**：`RunawayReason` 為 `Length`、`WhitespaceRun`、`WhitespaceRatio` 或 `LineTooLong`（§2.4，只有 CLI 會有）之一，帶進錯誤碼供診斷。

### 2.2 政策參數（明確傳參，不從 `usage_log` 推）

`RunawayPolicy`：
- `Full`：字數上限加退化偵測；
- `DegenerateOnly`：只做退化偵測；
- `Off`：不檢查。

`run_cli`、`run_cli_cancellable`、三個 API 串流函式（`stream_chat`、`stream_chat_models`、`stream_responses` 及其 `_windowed` 版本）都新增必填的 `policy` 參數，不給預設值，也不留沿用舊簽名的包裝。`UsageLog` 可能是 `None`，lane 的 `shape` 又固定寫成 `Oneshot`（`lanes/mod.rs:1040`），所以兩者都不拿來判斷政策。

| 呼叫 | 路徑 | 政策 |
|---|---|---|
| 角色聊天、GM 旁白、GM 建議（claude／grok／agy） | `lanes::run_turn` → `run_cli_cancellable`（`lanes/mod.rs:1015`） | `Full` |
| 角色聊天、GM（api／codex） | `dispatch::stream_turn_reporting_truncation` 收到 `PromptShape::Turn`（`commands/chat.rs:204,402`） | `Full` |
| 換幕摘要、開桌、翻譯、卡重構單發 | `stream_via_transport`，固定 `PromptShape::Oneshot`（`dispatch.rs:213-242`） | `DegenerateOnly`（A3） |
| 卡重構 claude session | `refactor_ai/session.rs:67` 直接 `run_cli` | `DegenerateOnly` |
| 生圖 | `PromptShape::Image`（`commands/image.rs:360-375`）、`generate_image` | `Off` |

`dispatch` 依 `PromptShape` 對應政策：Turn → Full、Oneshot → DegenerateOnly、Image → Off，同一個值傳給 CLI 與 API 兩邊。

智慧免費的正式呼叫點是 `SmartFreeEnv::send`（`dispatch.rs:311-325`）：`SmartFreeEnv` 加一欄 `policy`，在 `dispatch.rs:375-385` 建構時由 `shape` 對應出來，`send` 再傳給 `stream_chat_models`。

### 2.3 思考增量也做退化偵測〔作者裁決 2026-10-07〕

字數上限只算正文。另開一支只做退化偵測的 guard 吃思考增量，與正文那支分開計，思考的空白不會和正文的空白串在一起。

思考 guard 跟著政策走：
- `Full`、`DegenerateOnly`：才跑思考的退化偵測。
- `Off`：思考 guard 也不跑，例如生圖走 grok 時，空白的 `thought` 不會殺掉生圖。

- **CLI**：
  - claude 的 `CliLine::Thinking` 直接餵；
  - grok 的 `thought` 從 `Other` 改解析成 `CliLine::Thinking`（`cli/stream.rs:397-421`）。聊天傳 `thinking_to_delta=false`，畫面不受影響。單發卡重構（`thinking_to_delta=true`）若走 grok，進度字尾會開始顯示 grok 的思考，與 claude 一致。
- **API**：Chat Completions 取 `delta.reasoning`／`reasoning_content`，Responses 取 `response.reasoning*.delta`。這些欄位 `stall.rs:94-122` 已經在認。
- **不涵蓋**：codex、agy 的思考行目前沒有解析器，本案不新增。這些行仍會讓 120 秒停滯計時續命；思考本身退化成空白時，這兩家照樣攔不到。
- **理由**：這些增量會讓停滯計時續命。思考本身退化成空白時，現況同樣攔不住，而且畫面上看不到。

### 2.4 CLI 讀迴圈（`cli/runner.rs`）

- **正文檢查**：`CliLine::Delta` 累加前先過正文 guard（`runner.rs:446-449`）。
- **fallback 檢查**：在讀迴圈內收到成功的 `Done`（`runner.rs:455`），而且至今沒有任何正文增量時，就立刻把 `Done.text` 當候選全文過正文 guard，不等程序結束。
  - 觸發：當場跳出迴圈、殺程序，回 `Runaway`。
  - 沒觸發：記下「fallback 已檢查」。迴圈後採用 fallback 時（`runner.rs:503-511`）不再餵 guard，不會重複計數。
  - 這樣可以避免 CLI 送出失控的 `Done` 之後持續吐 stderr、不退出時，檢查永遠輪不到。
- **收場方式**：觸發就跳出迴圈，`kill_child_and_wait` 後回新的收場 `CliFinish::Runaway { partial, reason }`。不經 `?`、也不回 `Err`，所以不會掉進 lane 續聊失敗的重開重試（`lanes/mod.rs:1201-1216`）。
- **`run_cli`（單發）**：把 `Runaway` 轉成 `Err(invalid_data("AI_OUTPUT_RUNAWAY: reason=… chars=…"))`。不包成 `UiMsg::CliReplyError`：包了會變成 `TTMSG:`，再被 `ai_call_failure` 包成 `AI_CALL_FAILED`，前端只會顯示通用錯誤。
- **單行上限（A8）**：新的逐行讀取器 `CappedLines`，stdout 與 stderr 都換用。這是記憶體上限，不受 `RunawayPolicy` 影響，`Off` 也照樣生效。
  - **取消安全**：持久的 byte buffer 存在讀取器裡、放在 `select!` 外面。每次 poll 用 `fill_buf` 找換行、`consume` 已搬進 buffer 的位元組。`select!` 其他分支勝出時，已讀的位元組留在 buffer，不會遺失，跟現在的 `next_line()` 一樣安全。
  - **容量**：搬進 buffer 的當下就檢查容量，超過門檻立刻回超限，不先把整條讀完。
  - **與 `next_line()` 相同的規則**：行尾去掉 `\n` 或 `\r\n`；EOF 時沒有換行的殘段當最後一行交出；整行湊齊才做 UTF-8 解碼，跨塊切斷的多位元組字元不受影響，非法 UTF-8 一樣回 `InvalidData`。
  - **超限**：stdout 或 stderr 任一條超限，都當 `Runaway`（理由 `LineTooLong`）收場。
- **帳本**：已經收到的真實 usage 照實記，不延後也不撤銷，現有落帳時序不變；拿不到 usage 就不估、不記（A6）。

### 2.5 lane（`lanes/mod.rs`）

- **專用分支**：新增 `Ok(CliFinish::Runaway { .. })` 分支，先走 `settle_abort`，再 `return Err(碼)`。收尾效果與玩家按停止相同，不另造新行為：
  - grok 整條撤線；
  - claude 照抹私設並留 `pending_rewrite`；
  - agy 不抹不刪，留 `pending_rewrite`；
  - claude 和 agy 下一輪因 `pending_rewrite` 重開全量。
- **清理失敗**：`settle_abort` 失敗時直接回那個錯，同樣不重派送。
- **半截不交出**：回 `Err` 而不是 `Ok(aborted)`。GM 旁白不會進 `TurnGuard::abort` 把半截留給前端落檔（`commands/chat.rs:382-385`）；回合紀錄由 `TurnGuard` 照錯誤路徑收成已中止。

### 2.6 API（`transport/client.rs`、`transport/responses.rs`）

- **三條串流都掛**：`stream_chat_windowed`、`stream_chat_models_windowed`、`stream_responses_windowed`，在 `extract_delta` 之後掛正文 guard，在 reasoning 欄位掛思考 guard（§2.3）。
- **觸發後**：break、丟掉連線。已經收到的 usage 照現有流程記帳，之後**在 `outcome.failure` 之前**直接回碼。不然沒有 `[DONE]` 會被判成 `AI_INCOMPLETE_RESPONSE`（`client.rs:427`）。
- **智慧免費**：新增 `ApiFailure::runaway(reason)`，`stage = Stream`、`emitted_text = true`（失控必定已吐出內容，首塊就觸發也一樣），分類落在 Other。結果是不重送、不計入換模。`smart_free/call.rs:68-79` 的 `display()` 比照 `AI_STREAM_STALLED` 明確放行這個碼，不加 `AI_FREE_MODEL_BUSY`。

### 2.7 卡重構的全量重試

`commands/refactor.rs:174` 的 `result.ok()` 改成走 `refactor_session::degrade_unless_runaway`：錯誤以 `AI_OUTPUT_RUNAWAY:` 開頭時直接回錯；其他錯誤照舊退回全量單發。

### 2.8 錯誤碼與前端

- **後端白名單**：`dispatch::ai_call_failure` 加上 `AI_OUTPUT_RUNAWAY:`（`dispatch.rs:192-200`）。
- **前端**：
  - `src/shared/ui/ai-error.ts` 的 `FAILURE_CODES` 加 `/^(?:Error:\s*)?AI_OUTPUT_RUNAWAY:/` → `errOutputRunaway`；
  - `explainAiError` 的回傳型別聯集加上 `errOutputRunaway`；
  - 10 個主字典各加一句，zh-TW 為正典。草稿：「AI 回覆失控（連續空白或長度異常），已自動中止，這段沒有寫進故事。可以再送一次，或換個模型。」
- **前端流程不改**：invoke 失敗時半截從畫面消失、不落檔，彈 `TurnFailedDialog`，玩家那句退回輸入框（A4、A5）。

## 3. 定案

- **A1 偵測方式**：字數上限加退化偵測，不設總時間上限。空白亂碼幾秒內就停；時間上限會誤殺慢模型。〔作者裁決 2026-10-07〕
- **A2 字數上限**：30,000 字元。這個值是目前最長正常回合的 6 倍以上。〔作者裁決 2026-10-07〕
- **A3 單發呼叫**：只套退化偵測，不設字數上限。卡重構等長輸出才不會被誤殺。〔作者裁決 2026-10-07〕
- **A4 半截**：丟掉、當失敗，玩家那句退回輸入框。故事裡不會混進垃圾。〔作者裁決 2026-10-07〕
- **A5 重試**：不自動重試，包括 lane 重開與卡重構退回全量。prompt 誘發的退化，重試一次只會再燒一輪。〔作者裁決 2026-10-07〕
- **A6 帳本**：拿不到 usage 就不估、不記；已收到的真實 usage 照實記。這和停滯、按停止的處理一致。〔作者裁決 2026-10-07〕
- **A7 `max_tokens`**：API 請求不帶。OpenRouter 的 `max_tokens` 含推理 token，帶了會截斷正常回合。〔作者裁決 2026-10-07〕
- **A8 CLI 單行長度**：做單行上限，門檻 1MB。stdout 或 stderr 任一行超過就一律回 `Runaway`。這是記憶體上限，獨立於 `RunawayPolicy`，`Off` 也適用。〔作者裁決 2026-10-07〕

## 4. 測試與實測

- **`runaway.rs` 單元測試**：
  - 字數剛好到上限與超過上限；單一增量就超過上限。
  - 連續空白 1,999 不觸發、2,000 觸發。
  - 窗口未滿 4,000 字元時，開頭大量換行不觸發；滿窗後達 90% 才觸發。
  - 全形空白與換行都算空白。
  - 含縮排的正常長文不誤判。
  - 退化片段後接正常文字時，連續空白計數歸零。
  - 同一段文字整塊餵與逐字元拆塊餵，結果相同。
  - 政策 `DegenerateOnly` 不觸發 `Length`；`Off` 什麼都不觸發。
- **CLI 讀迴圈**（`cli/runner/tests.rs` 的假 CLI 腳本）：
  - 每行吐一個空白 text 事件吐不停 → 回 `Runaway`，程序被殺。
  - 零增量、`Done.text` 帶失控全文 → `Runaway`。
  - 失控的 `Done` 之後持續吐 stderr、程序不退出 → 當場 `Runaway`、程序被殺，不必等到 120 秒。
  - 正常的 `Done` fallback → 採用全文，guard 只計一次。
  - `Off` 政策下吐不停的空白 `thought`／Thinking → 不觸發。
  - `Off` 政策下吐超長單行 → 仍然超限。
  - grok `thought` 吐不停空白 → `Runaway`。
  - stdout 超長單行 → 超限；stderr 超長單行 → 超限。
  - stderr 插在半條 stdout 中間 → 拼回的行完整、一個位元組都不少。
  - CRLF 和無換行 EOF 的處理與 `next_line()` 相同。
  - `run_cli` 的錯誤字串以 `AI_OUTPUT_RUNAWAY:` 開頭，不是 `TTMSG:`。
- **lane**（`lanes/tests.rs`，打「已開線的續聊」）：
  - 失控時只派送一次，不重開重試；
  - grok 撤線；
  - claude 留 `pending_rewrite`；
  - agy 留 `pending_rewrite`；
  - 回錯誤碼；
  - `settle_abort` 失敗時也只派送一次。
- **卡重構**：`resume_stage` 失控 → 不走全量單發，派送一次。
- **API**（`transport/test_support.rs:72` 的腳本伺服器）：
  - `stream_chat`、`stream_chat_models`、`stream_responses` 持續送空白 content → 回 `AI_OUTPUT_RUNAWAY:`，不是 `AI_INCOMPLETE_RESPONSE`。
  - 持續送空白 reasoning → 同樣觸發。
  - `Off` 政策下持續送空白 reasoning → 不觸發。
  - 失控前已收到 usage → 帳本照實記一筆；沒收到 → 不記。
  - 智慧免費首塊即觸發 → 派送一次、不換模，`display()` 輸出原碼。
- **錯誤碼鏈**：
  - `ai_call_failure` 原樣放行；
  - 前端 `explainAiError` 對有、無 `Error:` 前綴的字串都對應到 `errOutputRunaway`；
  - `check:i18n` 與 `npm run verify` 全綠。
- **提交面**：角色聊天與 GM 旁白失控後，transcript、狀態都不寫入半截。指令層沒有 tauri mock，改在測試通道實測驗（逐字稿行數、state.json 前後比對）。
- **實測**（測試通道，測試 root 用指定的 ttroot，開 app 前先 ps 確認沒有其他實例）：
  - 真模型會留在角色裡、不照做，觸發靠本機假 chat/completions 端點（`base_url` 指向它，吐不停的空白或同一句），驗字數上限、退化偵測都會觸發。
  - 同時驗彈窗文字、玩家句退回輸入框、下一輪重開線正常。
  - claude 用 Sonnet、grok 各跑正常回合，確認不誤殺。
