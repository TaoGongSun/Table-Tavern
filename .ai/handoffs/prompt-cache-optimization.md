# Handoff: prompt-cache-optimization

## Current state
A（穩定前綴重構）、C（命中率量測）、B（Claude 顯式斷點）＋D（CLI 路徑量測）全部實作完成，cargo test 178 全綠（2026-08-03 本機重跑）。等實機驗收。

**A／B／C 只作用在 API 模式**：CLI 訂閱模式走 `cli::flatten_messages` 攤平後交給 CLI，前綴重構與顯式斷點都不經過，快取由 CLI 自己那端決定。使用者目前設定是 `transport=claude`，故 A／B 的驗收需先切到 API 模式；D 補上的是 CLI 這條路的命中率能見度。

## Completed
- 分析階段（2026-08-03 上午，唯讀）：快取現況、兩個打破前綴的設計、三塊方案已拍板（證據行號以 HEAD f8801e8 核對，詳見 git 歷史中本檔前一版）。
- **A 穩定前綴重構**（2026-08-03 下午，本次新完成）：
  - `assemble_messages`：世界書條目以 `partition` 拆 constant／keyword，constant 留在 system（transport.rs:181-199）；keyword 條目改為 events 迴圈後附加的一則獨立 user 訊息，標頭沿用「## 你知道的世界情報」（transport.rs:212-223）。
  - `assemble_gm_messages`：同樣拆分（transport.rs:263-281）；keyword 條目（標頭「## 世界書（只進你的上下文）」）與「## 目前狀態」合併成尾端一則獨立 user 訊息，世界書在前、狀態在後（transport.rs:322-353）。
  - 動態塊刻意不走 `push_merged`：獨立一則、維持語意邊界，不黏進最後一則發言；可能出現相鄰兩則 user，OpenAI-compatible API 接受，CLI 路徑 `flatten_messages` 攤平純文字不受影響。
  - 測試對齊：`table_state_reaches_the_gm_only` 改斷言狀態在尾端 user 訊息且 system 不含；新增 `keyword_entries_move_to_tail_message_constant_stay_in_system` 與快取友善驗收 `consecutive_rounds_share_verbatim_prefix_except_tail`——連續兩輪組裝、去掉尾端動態塊與最新事件後前綴逐字相同，GM 與角色路徑皆驗。
- **C 命中率量測**（2026-08-03，本次新完成）：
  - `chat_request_body` 抽出請求本體組裝：`include_usage` 時加 `"usage": {"include": true}`（transport.rs:728-744）；`stream_chat` 只在 base URL 含 `openrouter.ai` 時開啟（transport.rs:778）——其他 OpenAI-compatible 端點不認得頂層 "usage"，嚴格的（OpenAI 官方）會拒絕請求，故非 OpenRouter 請求形狀與加此功能前完全相同（同一 json! 構造）。
  - `PromptCacheUsage`＋`extract_usage`：解析 SSE 尾塊 `usage.prompt_tokens` 與 `usage.prompt_tokens_details.cached_tokens`；增量塊的 `"usage": null` 回 None，缺 details 記 0（transport.rs:699-723）。
  - 串流結束後 `eprintln!` 一行 `[prompt-cache] model=… prompt_tokens=… cached_tokens=… hit_rate=…%`（transport.rs:810-822）；正式 UI 位置仍待拍板。
  - **重要修正**：OpenRouter 官方文件明言「不支援回報寫入快取的 token 數」——先前分析假設的 cache_write_tokens 欄位不存在，只有讀取命中 cached_tokens（https://openrouter.ai/docs/use-cases/usage-accounting）。
  - 測試新增 3 條：`chat_request_body_adds_usage_only_when_asked_and_stays_bytewise_identical_otherwise`、`extract_usage_reads_final_chunk_and_ignores_delta_chunks`、`stream_chat_passes_usage_chunk_through_without_breaking_deltas`（mock SSE 含 usage 尾塊，增量不受影響）。
- **B Claude 顯式斷點**（2026-08-03，本次新完成，使用者拍板「現在做」）：
  - **實作方式與原計畫的差異**：不改 `ChatMessage` 型別（原計畫是 content 改 multipart 陣列）——multipart 轉換只發生在請求序列化邊界 `anthropic_messages`（transport.rs:731-748），組裝層／CLI 層／測試全部零波及，非 anthropic 模型逐位元不變也因此不證自明（同一 json! 構造路徑）。
  - `chat_request_body`：模型 id 以 `anthropic/` 開頭時，messages 轉 multipart（每則 content＝單一 text 分段）並標兩個 `cache_control: {"type": "ephemeral"}` 斷點（transport.rs:764-766）。
  - **斷點策略**：標在 system（角色卡／world.md／constant 條目，換卡前不變）與**最後一則 assistant**（其後只剩會變動的東西：可能被 push_merged 續寫的最後一則 user、動態塊、導演指示）。transcript 逐輪增長、斷點跟著前移；Anthropic 查快取會回看斷點前約 20 個 content block，前一輪的快取點仍在回看範圍內 → 逐輪增量命中。上限 4 個斷點，用 2 個。
  - 測試 1 條：`anthropic_models_get_multipart_content_with_two_breakpoints`（transport.rs:1611）——斷點位置恰為 [0, 最後 assistant]、multipart 文字照舊、非 anthropic 維持純字串 content、無 assistant（開桌第一輪）只標 system。
- **命中率落檔**（2026-08-03，使用者拍板：不做正式 UI、寫 log 隨時可查；日後介面／組裝改動若打破前綴，看 log 立刻現形）：
  - `append_usage_log`：一次呼叫一行，`data::local_timestamp()` 時間戳＋model＋prompt_tokens＋cached_tokens＋hit_rate，append 模式、寫檔失敗不影響聊天、無輪替（一行約百位元組）。
  - `stream_chat` 加 `usage_log: Option<&Path>` 參數；`stream_via_transport`（lib.rs API 分支）傳 `data_root/prompt-cache.log`——即「文件/TableTavern/prompt-cache.log」，與世界資料同目錄，好找。stderr 的 `[prompt-cache]` 行保留（終端機啟動時即時看）。
  - 測試：`stream_chat_passes_usage_chunk_through_without_breaking_deltas` 擴充——mock 串流後斷言 log 檔恰一行、含 model／token 數／hit_rate=60%。

- **D CLI 路徑量測**（2026-08-03，本次新完成——起因：使用者用 CLI 模式跑劇情後 log 一行都沒有，因為量測只掛在 API 那條）：
  - 四支 CLI 實測輸出確認（scratchpad 真實冒煙）：claude 的 `result`、codex 的 `turn.completed`、grok 的 `end` 事件都帶 usage；**agy 吃純文字輸出拿不到**，要換 `--output-format stream-json` 並重寫 `parse_agy_line`，使用者拍板「先做三支，agy 之後再說」。
  - **各家 input_tokens 語意不同，是這塊最容易算錯的地方**：claude／grok 的 `input_tokens` **不含**快取部分（claude 實測讀滿快取時 input_tokens=1、cache_read=4771；grok 實測 cache_read 146304 > input 31509），總輸入要加總；codex 的 `input_tokens` **已含** `cached_input_tokens`，再加就會虛報分母、低估命中率。`parse_claude_usage`／`parse_codex_usage`／`parse_grok_usage` 各自換算成統一語意的「總輸入／讀快取」（cli.rs:485-538）。
  - `CliLine` 刻意不動（13 處既有斷言零波及）：usage 走獨立抽取函式，`run_cli` 逐行呼叫，並以 `line.contains("\"usage\"")` 預檢——串流上千行只有收尾那行真的解析 JSON。
  - `UsageLog { path, transport, model, parse }` 打包落檔設定當 `run_cli` 的一個參數（避免簽名長出四個平行參數）；lib.rs 三支 CLI 分支各自帶入，agy 傳 None。
  - log 格式加 `transport=` 欄位（api／claude／codex／grok），兩條路共用同一份檔案；命中率計算收進 `PromptCacheUsage::hit_rate()` 兩邊共用。
  - 測試新增 4 條：三支 parser 各一條（樣本取自真實冒煙輸出，鎖住換算語意與缺欄位當 0），加端到端——既有假 CLI 測試的 result 行補上 usage，斷言 log 恰一行且內容為 `transport=claude model=sonnet prompt_tokens=100 cached_tokens=99 hit_rate=99%`。

## Verification
- `cargo test`：178 passed; 0 failed（2026-08-03 本機，含前一手雲端容器寫的 A／B／C 測試重跑）。
- CLI 用量欄位為實機冒煙查證，非文件推測：claude 同段 system prompt 連跑兩次，第一次 cache_creation=4771／cache_read=0（$0.0179），第二次 cache_creation=0／cache_read=4771（$0.0015）——快取在 CLI 端本來就自動運作。

## Remaining / Next action
1. **實機驗收 D（CLI 量測）**：維持現在的 claude CLI 模式開桌跑兩輪以上，看「文件/TableTavern/prompt-cache.log」出現 `transport=claude` 行且第二輪起 hit_rate 明顯 >0。
2. **實機驗收 A＋B＋C**（需切到 API 模式＋貼 OpenRouter key，會花錢）：看 `transport=api` 行的 cached_tokens——隱式快取模型（GPT／DeepSeek／Gemini 2.5／Grok）與 Claude 系（anthropic/，靠 B 的顯式斷點）都應 >0；同時比對條目與狀態搬尾端後的敘事品質，若明顯變差，該項回退進 system 並在任務檔記錄取捨。注意 Anthropic 顯式快取有最低門檻（約 1024 tokens），太小的桌可能不寫快取，屬預期。
3. **agy 量測**（未拍板）：要換 `--output-format stream-json` 並重寫 `parse_agy_line`（usage 在 `result` 事件的 `cache_read_tokens`，格式用 `event` 欄位而非 `type`）。風險是改壞 agy 那條線的回覆解析，需實測串流輸出確認回覆不斷行。

## Constraints
同 tasks 檔。另注意：constant 條目與角色卡留在 system 是刻意的（穩定且需要高遵循度）；A 實測若條目放尾端遵循度明顯變差，該項回退並在任務檔記錄取捨。
