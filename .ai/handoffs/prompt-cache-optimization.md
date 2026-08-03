# Handoff: prompt-cache-optimization

## Current state
A（穩定前綴重構）、C（命中率量測）、B（Claude 顯式斷點）三塊全部實作完成＋自驗綠，命中率並已落檔（使用者拍板：不做正式 UI，寫 log）。cargo test 168 全綠（2026-08-03，Cowork 雲端會話）。程式面收工，等實機驗收：開桌看 A 的遵循度、看 `[prompt-cache]` 命中率（隱式模型與 Claude 系都應 >0）。

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

## Verification
- `cargo test`：168 passed; 0 failed（2026-08-03，雲端 Linux 容器、rustc 1.95.0）。A 2 條、C 3 條、B 1 條新測試逐一確認 ok（名稱見上）。
- 注意：本次在雲端容器編譯測試，未在本機跑過；下一手開工時本機重跑 `cargo test` 確認一次即可。

## Remaining / Next action
1. **實機驗收 A＋B＋C**（使用者本人，唯一剩餘項）：直接開 app 玩即可，事後看「文件/TableTavern/prompt-cache.log」——同一桌連續兩輪起，隱式快取模型（GPT／DeepSeek／Gemini 2.5／Grok）與 Claude 系（anthropic/，靠 B 的顯式斷點）的 cached_tokens 都應明顯大於 0；同時比對條目與狀態搬尾端後的敘事品質，若明顯變差，該項回退進 system 並在任務檔記錄取捨。注意 Anthropic 顯式快取有最低門檻（約 1024 tokens），太小的桌可能不寫快取，屬預期。驗收通過即可結案。

## Constraints
同 tasks 檔。另注意：constant 條目與角色卡留在 system 是刻意的（穩定且需要高遵循度）；A 實測若條目放尾端遵循度明顯變差，該項回退並在任務檔記錄取捨。
