# Handoff: prompt-cache-optimization

## Current state
分析完成（2026-08-03，Cowork 雲端對話），程式未動工。快取現況與三塊方案方向已拍板，證據行號以當日本機 HEAD f8801e8 核對。

## Completed
- 唯讀分析（無程式改動）：
  - `stream_chat` 請求僅 model／messages／stream，無 `cache_control`（src-tauri/src/transport.rs:687-696）。
  - 世界書 keyword 條目以最近 4 則事件的關鍵字掃描決定進出（transport.rs:108-125），並拼進 system prompt：角色路徑 transport.rs:180-190、GM 路徑 transport.rs:242-252 → 條目翻動即從 context 第一段打破前綴。
  - GM system prompt 內嵌 `state.table` 目前狀態（transport.rs:253-267），每輪旁白後更新 → GM 幾乎每輪前綴不同、全額重算。
  - `extract_delta` 只取 delta.content，usage 塊被丟棄（transport.rs:654-668）；OpenRouter 串流要拿 usage 需請求加 `"usage": {"include": true}`。
  - `ChatMessage.content` 為純字串（transport.rs:80-84），做 B（Claude 顯式斷點）前需先支援 multipart content 陣列。
  - OpenRouter 快取行為：OpenAI／DeepSeek／Grok／Gemini 2.5 隱式自動；Anthropic 需顯式 `cache_control`，未標＝完全不快取（https://openrouter.ai/docs/features/prompt-caching）。
  - 角色路徑（非 GM）在卡片與條目不變時 system 穩定、transcript append-only，`push_merged` 只動最後一則尾巴（transport.rs:94-102），前綴形狀本來就快取友善。

## Verification
- 純閱讀分析，未跑任何指令；上列行號皆對 HEAD f8801e8 的檔案逐一核過。

## Remaining / Next action
依 A→C→B（詳見 tasks 檔）。A 的具體開工步驟：

1. `transport.rs`：`assemble_gm_messages` 把「目前狀態」區塊、`assemble_messages`／`assemble_gm_messages` 把 keyword 觸發（非 constant）的世界書條目，從 system 字串搬出，改為 events 迴圈後附加的一則 user 訊息（標頭沿用「## 你知道的世界情報」等既有措辭，跟著搬）。
2. 注意 `push_merged`：相鄰同 role 會合併，動態塊直接 push 成 user 會黏進最後一則使用者發言——決定要合併還是繞過 `push_merged` 保持獨立一則（建議獨立，維持語意邊界，但需確認 user/assistant 交錯不被打破）。
3. 既有測試對齊：transport.rs tests 中斷言 system 內容含世界書／狀態者，改斷言尾端訊息；加一條「連續兩輪組裝、去尾後前綴逐字相同」的快取友善測試。
4. `cargo test` 綠後做 C：`stream_chat` 請求加 usage include，SSE 尾塊解析 `prompt_tokens_details`（cached_tokens／cache_write_tokens），先 eprintln!/log 驗證 A 效果。
5. B 最後：content 改 multipart、僅模型 id 含 `anthropic/` 時標斷點；其他模型序列化結果需與現狀逐位元相同（加測試）。

## Constraints
同 tasks 檔。另注意：constant 條目與角色卡留在 system 是刻意的（穩定且需要高遵循度）；A 實測若條目放尾端遵循度明顯變差，該項回退並在任務檔記錄取捨。
