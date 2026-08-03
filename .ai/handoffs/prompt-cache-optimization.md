# Handoff: prompt-cache-optimization

## Current state
A（穩定前綴重構）實作完成＋自驗綠（2026-08-03，Cowork 雲端會話；cargo test 164 全綠，較前次 162 多出本次新增的 2 條測試）。C（命中率量測）未動工。

## Completed
- 分析階段（2026-08-03 上午，唯讀）：快取現況、兩個打破前綴的設計、三塊方案已拍板（證據行號以 HEAD f8801e8 核對，詳見 git 歷史中本檔前一版）。
- **A 穩定前綴重構**（2026-08-03 下午，本次新完成）：
  - `assemble_messages`：世界書條目以 `partition` 拆 constant／keyword，constant 留在 system（transport.rs:181-199）；keyword 條目改為 events 迴圈後附加的一則獨立 user 訊息，標頭沿用「## 你知道的世界情報」（transport.rs:212-223）。
  - `assemble_gm_messages`：同樣拆分（transport.rs:263-281）；keyword 條目（標頭「## 世界書（只進你的上下文）」）與「## 目前狀態」合併成尾端一則獨立 user 訊息，世界書在前、狀態在後（transport.rs:322-353）。
  - 動態塊刻意不走 `push_merged`：獨立一則、維持語意邊界，不黏進最後一則發言；可能出現相鄰兩則 user，OpenAI-compatible API 接受，CLI 路徑 `flatten_messages` 攤平純文字不受影響。
  - 測試對齊：`table_state_reaches_the_gm_only` 改斷言狀態在尾端 user 訊息且 system 不含；新增 `keyword_entries_move_to_tail_message_constant_stay_in_system`（transport.rs:1539）與快取友善驗收 `consecutive_rounds_share_verbatim_prefix_except_tail`——連續兩輪組裝、去掉尾端動態塊與最新事件後前綴逐字相同，GM 與角色路徑皆驗（transport.rs:1581）。

## Verification
- `cargo test`：164 passed; 0 failed（2026-08-03，雲端 Linux 容器、rustc 1.95.0）。三條相關測試逐一確認 ok：`keyword_entries_move_to_tail_message_constant_stay_in_system`、`consecutive_rounds_share_verbatim_prefix_except_tail`、`table_state_reaches_the_gm_only`。
- 注意：本次在雲端容器編譯測試，未在本機跑過；下一手開工時本機重跑 `cargo test` 確認一次即可。

## Remaining / Next action
1. **實測遵循度**（A 的待拍板項）：實際開桌比對條目與狀態搬尾端後 GM／角色的敘事品質；若明顯變差，該項回退進 system 並在任務檔記錄取捨。
2. **C 命中率量測**：`stream_chat` 請求加 `"usage": {"include": true}`（transport.rs:733-737 的 json! 塊），SSE 尾塊解析 `usage.prompt_tokens_details`（cached_tokens／cache_write_tokens）——`extract_delta` 目前丟棄 usage 塊（transport.rs:699 起），需另接解析；先 eprintln!/log 驗證 A 效果，正式 UI 待拍板。
3. **B Claude 顯式斷點**（最後）：`ChatMessage.content` 改支援 multipart 陣列，僅模型 id 含 `anthropic/` 時在穩定前綴尾標 `cache_control`；其他模型序列化結果需與現狀逐位元相同（加測試）。

## Constraints
同 tasks 檔。另注意：constant 條目與角色卡留在 system 是刻意的（穩定且需要高遵循度）；A 實測若條目放尾端遵循度明顯變差，該項回退並在任務檔記錄取捨。
