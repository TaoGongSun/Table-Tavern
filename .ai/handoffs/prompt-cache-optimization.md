# Handoff: prompt-cache-optimization

## Current state
A（穩定前綴重構）、C（命中率量測）、B（Claude 顯式斷點）＋D（CLI 路徑量測）全部實作完成；resume 續聊架構**包 1（session 檔操作模組）＋包 2（resume 流程地基）＋包 3（凍結快照補丁＋追平＋原因代碼）＋包 4（log 升級 JSONL＋診斷標籤）＋包 5（GM 旁白＋點名合併）完成**，cargo test 213 全綠、零警告（2026-08-04 本機）。claude 訂閱模式的角色對話與 GM 呼叫已全部走 resume 續聊線，改卡／改世界書當輪不再重開線；GM 每輪推進由兩次 CLI 呼叫（旁白＋點名）併成一次。

**2026-08-04 Opus 四輪真桌驗收全過＝包 1–5 架構驗收通過**（見 Verification）：2–4 輪 diag=ok、85.3／87.2／88.3%，讀到量與理論可中量每輪只差 2 個 token＝「只有本輪新內容沒中」的理想形狀，零 cache-skipped。Sonnet 兩場的爛數字全歸 CLI 官方 bug。官方 issue **不發**（2026-08-04 使用者拍板）。

**包 6 額度分頁已實作完成、尚未實機驗收**（2026-08-04；使用者當輪指定「先做完不驗收，驗收另開對話」）。程式面全綠（cargo test 217、clippy 無新增、npm build＋check:i18n），**沒有開過 app 看畫面、沒有跑過真呼叫**——待驗清單見 Verification 的「包 6 待驗收」。

**2026-08-04 凌晨真桌驗收兩場（各四輪 GM 線）未達標，根因鎖死在 claude CLI 本身**：CLI session 檔逐筆 usage 證明**單數輪請求完全不帶快取標記**（讀寫皆 0、整句全額計費），雙數輪正常寫讀；第 4 輪精準讀到第 2 輪寫入的 18,020 tokens＝app 組的前綴穩定、包 2／3／5 機制無誤（log 的 prefix-broken 標籤在此情境是誤導，實為前一輪沒寫）。乾淨環境同旗標三連發重現「寫入隔輪消失」，排除 app 傳參與環境變數。社群修復（本機 proxy 攔請求／regex 改 cli.js）否決：proxy 位在玩家憑證路徑上、兩者都綁 CLI 版本養維護債、且修的是「前綴被改壞」變體，對我們「隔輪不帶標記」對症存疑。

**2026-08-04 深夜追加查證（CLI 升 2.1.220 重測＋模型對照＋社群）**：升級後重跑四輪 0／0／64.3／63.7（整場 38%，前場 14%）——減輕沒根除，`cache-skipped` 標籤首戰即準確抓到問題輪。**決定性發現：同版 CLI 同旗標三連發，Opus 全命中、Sonnet 首發整包不帶標記＝bug 只咬 Sonnet 系**。佐證：官方 repo issue #29966（open、有 assignee）拍到機制——CLI 內部 `enablePromptCaching ?? false` 硬編碼＋按模型判斷的 `YWq(model)`，Sonnet 路徑零斷點、Opus 正常。社群同族 issue 多數被關（含 not planned），無官方時程。換 Opus 不划算：輸出單價 1.67 倍且輸出佔帳單大頭，Opus 快取滿分（估 $0.47/場）仍貴過壞快取的 Sonnet（實測 $0.34/場）；Sonnet 修好估 $0.28/場。降版 2.1.68（#34629 稱可解）不採：太舊。

**2026-08-03 晚拍板收束範圍：只打 CLI（claude 訂閱）這條路**——OpenRouter／API 模式、1h 快取、模型體質表、開桌預熱、GM 全代演全部出局。A／B／C 的 API 驗收擱置。

**CLI 快取行為已用七個對照實驗鎖死**（同日晚，用使用者訂閱跑 ~15 次迷你呼叫，材料與 log 在 scratchpad/cache-probe/）：

| 實驗 | 設計 | 結果 |
|---|---|---|
| E1 | 固定 system＋3 次不同短 prompt | 首輪就建（create 6857）；r2/r3 read=6714＝system 段全中 |
| E2 | prompt 內模擬歷史逐輪 append | read 恆＝system 段；prompt 整段每輪全額重寫（塊級匹配） |
| E3 | 快取熱後隔 6 分 24 秒 | read=0 全額重建 → 壽命確為 5 分鐘級 |
| E4 | system 逐輪 append | read=0 → system 塊同樣「變一字全滅」，歷史塞 system 沒用 |
| E5 | `--resume` 續聊、不重帶 system | 增量命中完美（r3 create=25），但 system 掉回預設、斷一次 |
| E6 | `--resume`＋每輪重帶同一份 `--system-prompt` | **r2/r3 read 全中、create=17（只有新句）＝99.7% 命中** |
| E7 | resume＋system 改一個詞 | read=0 整條全滅 → 凍結補丁在續聊下仍必要 |

**結論：單發 `-p`（現行 §8.1 無狀態架構）的命中率天花板＝逐字不變的 system 佔比**（歷史在 prompt 裡，塊級匹配下每輪全額付＋全額重寫 1.25x，中期必然 <35%）。要達成使用者目標 66%+，**必須翻案 §8.1 改用 `--resume` 續聊**：每輪只送新訊息＋重帶同一份凍結 system，實測就是「只有最後一句沒中」的理想形狀。

三輪實測全 0% 的解答：單發模式下 prompt 段結構性不可命中＋GM／各角色多線分散＋5 分鐘壽命，三者疊加。「第一輪不建」在乾淨環境不重現（E1／E3／E6 首輪都正常建）——本 Claude Code 會話環境帶 `ANTHROPIC_AUTH_TOKEN`／`ANTHROPIC_BASE_URL` 會蓋掉訂閱登入（實驗腳本需 `env -u` 清除），先前 scratchpad 四輪的異常節奏疑為類似環境污染，不再追。

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
  - **第一版落檔漏了診斷關鍵欄位，已補**（2026-08-03 實測後）：只記讀快取時，命中率 0 分不出「根本沒建」與「建了但讀不到」。`PromptCacheUsage` 加 `created_tokens`（claude `cache_creation_input_tokens`／codex `cache_write_input_tokens`／grok 與 OpenRouter 不回報恆 0）；時間戳改秒級（`data::local_timestamp_seconds`，由 `local_time_parts` 與既有分鐘級共用，存檔用的 `local_timestamp` 格式不變），分鐘精度看不出是否踩到 5 分鐘過期線。
  - 測試新增 4 條：三支 parser 各一條（樣本取自真實冒煙輸出，鎖住換算語意與缺欄位當 0），加端到端——既有假 CLI 測試的 result 行補上 usage，斷言 log 恰一行且內容為 `transport=claude model=sonnet prompt_tokens=100 cached_tokens=99 hit_rate=99%`。

- **包 1 session 檔操作模組**（2026-08-03 晚，本次新完成；規格主線寫、實作外派 codex terra、主線親讀驗收）：
  - 新檔 `src-tauri/src/session_file.rs`：`SessionFile` 全程 `serde_json::Value` 保留未知欄位；8 個純函式——`session_file_path`（munge：非 ASCII 英數一律轉 `-`）、`parse`（逐行驗證：type／uuid／parentUuid 鏈／user content 字串／assistant content 陣列，錯誤含行號）、`serialize`、`erase_user_segment`（片段必須恰出現一次，0 或 ≥2 次 Err）、`prefix_last_assistant`（最後一條 assistant 首個 text 分段前插 prefix，冪等）、`truncate_from`（截指定對話行與其後所有行含雜項行）、`write_atomic`（同目錄暫存檔＋rename＋回讀逐行比對）、`load`。
  - `lib.rs:8` 加 `mod session_file;`；模組頂 `#![allow(dead_code)]`（包 2 接線後移除）。
  - 單測 10 條（假 JSONL 照真檔格式、雜項行穿插）：munge、快樂路徑、壞 JSON／斷鏈／content 型錯三失敗路徑、round-trip、抹段（含 0 次／2 次 Err）、前綴（改最後一條＋冪等＋無 assistant Err）、截尾（尾隨雜項行一併截）、原子寫實檔覆寫＋回讀。

- **包 3 凍結快照＋補丁＋追平＋原因代碼**（2026-08-03，本次新完成；規格主線寫〔scratchpad/pkg3-spec.md，session 失效後以程式碼為準〕、實作外派 codex terra、主線親讀 diff 驗收）：
  - 新檔 `snapshot_patch.rs`：`render_patch(applied, current)` 純函式——素材切塊（`## `／`### ` 標題行為界，前導指示自成一塊，同名標題靠出現序配對），變更／新增段落照 current 順序原文輸出在「## 設定更新」標頭下，移除段落列於「（已移除段落：…）」，無變化回 None。單測 6 條。
  - `lanes.rs`：`plan_turn` 刪「快照不合＝重開」——對齊檢查全過後看距上輪呼叫秒差（`last_call_epoch`，取代原 `last_call_at` 字串；舊 lanes.json 缺欄位 parse 失敗＝全部重開一次，既定降級）：>300 秒＝快取已死，直接把新素材換進 `--system-prompt` 追平（零成本）；≤300 秒＝system 沿用舊快照逐字不變、變動走補丁附在回合尾段最前。LaneState 新增 `applied`（已傳達素材＝快照＋歷來補丁合成），補丁 diff 基準是 applied 不是 snapshot，已送過的補丁不重送。
  - **原因代碼落 log**（額度分頁地基）：`log_lane_action` 追加到 prompt-cache.log（寫失敗靜默）——`action=reopen reason=first-turn|pending-rewrite|scene-changed|history-rewound|history-edited|reply-diverged|resume-failed`、`action=patch`、`action=rebase`、`action=drop-lane reason=rewrite-failed`；正常續聊不記（用量行由 cli.rs 照舊）。
  - 測試新增 8 條：snapshot_patch 6＋plan_turn 補丁／追平 2，另既有 e2e 之外新增一條端到端（假 CLI）：素材改動→續聊帶舊 system＋prompt 含「## 設定更新」；手改 lanes.json epoch 減 3600→追平帶新 system、無補丁；log 斷言 reopen／patch／rebase 三種行都在。
  - codex 越界順手跑了 cargo fmt 動到六個範圍外檔案，主線已還原（`git checkout` 六檔＋lib.rs 重做最小 diff），實質改動只有規格內三檔。
- **包 2 resume 流程地基**（2026-08-03 深夜，本次新完成；主線直寫）：
  - **案 C 核心假設先實測鎖死**（3 次 haiku 迷你呼叫，材料在 scratchpad/rewrite-probe/）：改寫過的 session 檔（抹段＋補「狐狸：」前綴）resume **照樣接受**，且只滅尾端快取——r1 開線 create=8865；改檔後 r2 read=8717／create=751；r3 read=9468／create=299＝理想增量形狀。另證實 resume 沿用同一 session id、續寫同一檔（--fork-session 才分叉），故 id 由本程式產生（--session-id）、不需捕捉。
  - `lanes.rs`（新檔）：`lanes.json`（worlds/<id>/ 下，壞檔＝重開線）記每線 session_id／scene／水位 sent_events／FNV-1a 指紋 sent_hash／凍結快照 snapshot／pending_rewrite／expected_reply／last_call_at。`plan_turn` 一項對不上就重開：pending 未清（上輪中途崩潰）、換場、快照變動、水位超前、已送段指紋不合、回覆事件沒落檔或被改。`run_turn`：呼叫前先落 pending_rewrite（崩潰安全）→ CLI → 回合後抹寫（機密段＋名字前綴，原子寫＋回讀）→ 落 expected_reply；續聊呼叫失敗同輪內自動降級重開全量，抹寫失敗丟線。
  - `transport.rs`：`chars_lane_system`（中性扮演引擎指示＋全公開卡＋玩家卡＋Public constant）／`chars_lane_turn`（公開 keyword 條目＋機密段〔私設＋Characters 限定條目，含 constant〕＋本輪指定，機密段回傳供抹寫）／`gm_lane_system`／`gm_lane_turn`／`lane_event_line`；GM 的 system 與動態塊抽成 `gm_system_prompt`／`gm_dynamic_block` 與單發共用，單發組裝零行為變化。
  - `cli.rs`：`claude_session_args`（同組旗標、無 --no-session-persistence，Open 帶 --session-id／Resume 帶 --resume；旗標組合已在 rewrite-probe 以真 CLI 驗過）。`session_file.rs`：`find_user_line_with_segment`（恰一行才准抹）；模組級 dead_code 移除，僅 `truncate_from`（undo 截尾，後續包）單獨保留。
  - `lib.rs`：`chat_with_character`／`gm_narrate`／`gm_suggest_speaker` 在 transport=claude 時分流 lane（chars／gm／gm+echo None）；`claude_cli_envs`／`prepare_claude_call`／`gm_materials`／`load_active_cards` 抽共用，舊 `assemble_gm` 併入。前端已核對：回覆原樣落 transcript（App.tsx replyOnce／gmNarrate 的 `text: full`），echo 逐字對點成立。
  - 測試新增 13 條：transport lane 組裝 4（快照只含共通素材、機密段恰出現一次且抹後公開內容仍在、GM 快照素材、事件行格式）、cli 旗標 1、session_file 定位 1、lanes 7（指紋、UUID v4、plan 決策矩陣、echo 跳過／分岔、prompt 形狀、端到端假 CLI：開線→抹寫→續聊只送增量→改字自動重開→session 檔被刪同輪降級重開）。

- **包 4 log 升級**（2026-08-04，本次新完成；主線直寫）：
  - 新檔 `usage_log.rs`：log 檔改 `prompt-cache.jsonl`（原 `prompt-cache.log` 純文字格式作廢，舊檔留在資料目錄不動、包 6 只讀 .jsonl）。**一次呼叫一行 JSONL**，線的動作與該次用量寫在同一筆——原本分成「用量行」與「線動作行」兩種行，命中率為什麼掉得靠時間戳自己接，接錯就誤診（三輪 0% 的誤判即出於此）。
  - 欄位：ts（秒級）／transport／model／diag／lane／reason／patched／rebased／prompt_tokens／cached_tokens／created_tokens／output_tokens／hit_rate／cost_usd／expected_cached／age_secs／system_tokens／system_hash。沒發生的旗標與沒有的值不佔位（省略而非寫 null）。
  - **診斷標籤定死七個**（`Diag` enum，純規則判定；每個標籤對一句玩家中文，表在 `usage_log.rs` 模組頂註解，包 6 照它配 i18n）：`ok`／`warmup`（重開，reason 帶包 3 的七種原因代碼）／`expired`（age > 300 秒）／`prefix-broken`（該中沒中，cached 對 expected 差一成以上）／`no-cache`（cached 與 created 皆 0）／`single`（API／codex／grok 單發，不做續聊診斷）／`drop-lane`（抹寫失敗丟線）。
  - **判定不從 token 反推**：重開與否、隔幾秒、上輪送多少，全是 app 自己的決策，`LaneContext` 帶著走。原規格的「前綴斷於第幾段」在續聊架構下只剩 system／對話兩段，改用 `expected_cached` 對 `cached_tokens`（差多少）＋ `system_tokens` 粗估（cached 接近它＝只有設定段中）表達，不另建分段 hash 框架。
  - 「上輪總輸入」＝這輪的理論可中量，存進 lanes.json 的 `last_prompt_tokens`（`#[serde(default)]`，舊檔不觸發重開）；`run_cli` 經 `UsageLog.prompt_tokens_out`（AtomicU64，跨 await 需 Sync）回填給 lane。
  - `PromptCacheUsage` 補 `output_tokens`＋`cost_usd`（claude 的 `total_cost_usd`，四支只有它直接回報金額；其餘 None 由包 6 靠有值的行加總）。
  - 測試新增 3 條（usage_log：七標籤判定矩陣、JSONL 行形狀含 lane／cost／省略旗標、token 粗估與 hash），改寫 3 條既有（cli e2e、transport stream_chat、lanes 補丁／追平 e2e 改斷言三筆 JSONL：warmup+first-turn／ok+patched+expected_cached=100／expired+rebased+age≥3600）。

- **包 5 GM 旁白＋點名合併**（2026-08-04，本次新完成；主線直寫）：
  - `transport.rs`：`narrate_instruction` 加 roster／player 參數——名單非空時指示尾端追加「圍欄之後最後另起一行輸出『下一位：〈名字〉』」（含玩家哨兵；名單空＝純世界書開局退回純旁白，不要求點名行）；新增 `extract_next_speaker`（與 extract_state_block 同族：只認整行、行首「下一位」／「Next」＋冒號，掃到多行取最後一行，剝行後回傳點名原文＋顯示文字；行首普通英文 Next 無冒號不誤判）；`suggest_instruction` 刪除，`pick_speaker` 留用對名字。
  - `lib.rs`：`gm_narrate` 回傳改 `GmNarration { text, next }`——剝完狀態欄再剝點名行，`pick_speaker` 對回角色 id（玩家哨兵原樣、對不上＝None 不當錯誤）；`gm_suggest_speaker` 命令刪除。claude lane 與單發兩條路同一份指示與解析。
  - `lanes.rs`：旁白 echo 的 expected_reply 同步改「剝狀態欄＋剝點名行」（與前端落 transcript 的顯示文字逐字一致，續聊對點才成立）；`ReplyEcho::None` 唯一使用者是點名，隨之刪除（`expected_reply_for` 收掉 Option）。
  - `App.tsx`：抽 `narrateOnce`（串流旁白→落 transcript→回傳 next），旁白鈕沿用但忽略 next（讓玩家自己決定下一步）；`gmAdvance` 迴圈改「旁白→點名紀錄→角色接話」，GM 沒點名＝就地停下，輪到玩家／每回合上限照舊。GM 每輪推進少打一次 CLI 呼叫，session 尾巴也少一組「指示→名字」。
  - 11 語系 `gmAdvanceHint` 改述「先旁白再點名」。測試：transport 新增 `extract_next_speaker` 4 情境＋旁白指示驗名單／哨兵／空名單退回，lanes 旁白 echo 測試補點名行。
- **包 7 保溫 ping**（2026-08-04，本次新完成；主線直寫）：
  - `lanes::keepalive`：對每條「距上輪 180–300 秒、pending 已清」的線送一則極短訊息（`PING_PROMPT`，兼作截尾定位片段），用該線自己的模型與凍結快照走 `--resume`；成功後 `truncate_ping` 把問答從 session 檔截掉（`session_file::truncate_from`，其 `#[allow(dead_code)]` 隨之移除）並把 `last_call_epoch` 推到現在。ping 失敗只是不保溫（不動線）；**截尾失敗才丟線**（`drop-lane reason=ping-truncate-failed`）——留著垃圾問答會讓下次 ping 的定位片段出現兩次而永遠失敗，丟線可自我修復。
  - 成本量級（拍板依據）：ping 一次＝讀一次整包快取，約為讓它過期重建的 **1/12**；連三次仍划算，48 分鐘內回來都比放它死掉便宜。
  - `usage_log`：第九個診斷標籤 `ping`（`LaneContext.ping`，判定置於最前），保溫花費與劇情輪分開統計，包 6 照此分項顯示。
  - `lib.rs`：`keepalive_lanes` 命令（非 claude 傳輸直接回 0，前端據此靜靜停掉）。
  - `App.tsx`：30 秒一次的計時器——距上次推進滿 3.5 分鐘、視窗在前景、沒有生成中才發；連三次（約 12 分鐘）沒等到玩家推進就停手，紀錄同時超過 8000 字元才亮 `sceneAwayHint`。任何一次真正推進（角色接話／旁白／換幕）都會 `noteTurnDone()` 重置節奏並收掉提醒。視窗不在前景一律不發——人不在還持續扣錢是最糟的情況。
  - 測試：lanes e2e 1 條（剛呼叫完不發／200 秒發且截尾後檔案逐字還原／過期不發／pending 不發／ping 後續聊回覆編號仍是 2＝真的沒留痕跡）＋usage_log ping 標籤 1 條；順手把假 CLI 建置抽成 `fake_claude` helper 供兩個 e2e 共用。
- **包 6 額度分頁 UI**（2026-08-04，本次新完成；主線直寫，9 語系翻譯外包 codex luna）：
  - **地基**：`usage_log::append_call`／`append_event` 加 `world` 欄（None＝開桌生成等不屬於任何一桌的呼叫，與加欄前的舊行同樣歸「未標桌」）；桌別一路由 `cli::UsageLog.world`／`transport::stream_chat(world)` 帶進來，`stream_via_transport` 多一個 `world: Option<&str>` 參數（七個呼叫點：聊天／旁白／換場摘要／生圖帶桌，三個開桌生成傳 None）。`parse_grok_usage` 補讀 `end` 事件頂層 `total_cost_usd`。
  - **agy 也要看得見**：agy 拿不到任何用量，改在呼叫成功後寫一行 `unreported:true`（`usage_log::append_unreported`），分頁才顯示得出「輪數＋這個來源不回報用量」——否則 agy 整條路在分頁上等於不存在。
  - 新檔 `usage_report.rs`：`summarize(log, scope, names, in_use)` 純函式，一次掃完 JSONL 產出 `{worlds, rows, total, ping, diags, latest}`。桌下拉不受選桌影響（全檔統計）、`scope=None` 即全桌總計；ping 自成一列不混進劇情輪；沒有 `model` 欄的線事件（丟線）只進診斷統計不算一輪；壞行跳過。命中率＝讀到快取／總輸入。
  - **金額只轉述**：把各 CLI 自己回報的 `cost_usd` 加總，缺值的輪次標 `cost_partial`（前端顯示 `≥ $x.xxx`），app 不自算牌價、不建價目表——分不出玩家是訂閱制還是 API 計費。
  - `lib.rs`：`usage_report` 命令（讀不到檔就當空檔）＋`current_models` 判「使用中」（照解析後真正傳給連線的模型字串，與 log 同一把尺）。
  - `App.tsx`：`UsageTab` 元件掛在設定視窗第三個分頁「額度」（外觀／AI／額度／作者），預設看當前桌。**收合狀態只給三件事**（2026-08-04 使用者拍板「剛打開盡量簡潔」）：桌下拉→一行「總 token・約 $x」→一條長條圖（綠＝讀到快取、灰＝全額，百分比印在段內；段窄於 18% 就只留顏色）。長條與花費都含保溫（那也是真的花掉的錢）。狀況非綠燈時多一行燈號句，正常就不佔版面。
  - **下拉列出 app 現有的每一桌**（沒紀錄的照列、進去顯示 0），不是只列紀錄檔裡出現過的桌——後者會讓剛開的桌落不到選項上，select 退回顯示第一項，看起來像「選了總計卻是空的」（2026-08-04 使用者實機回報）。對不上現有桌的紀錄只剩兩種且都有看得懂的名字：`id` 空＝**開桌生成**（半途放棄、沒生出桌的嘗試），`id` 有值但查無此桌＝**已刪掉的桌**。
  - **開桌生成的額度算進生出來的那桌**（2026-08-04 使用者拍板：錢是為那桌花的）：那幾次呼叫落檔時桌還沒建出來，`generate_table_expand` 建好桌後呼叫 `usage_log::assign_pending_world` 把沒有 `world` 欄的行補上桌 id（暫存檔＋rename）。半途放棄的舊嘗試若還留著未認領的行，會一起認到下一張生出來的桌名下——同樣是開桌花的錢。開發期沒帶桌別的舊行已於 2026-08-04 一次性改寫成「新的一桌」（使用者測試都在該桌），玩家不會遇到。
  - 細項收進 `<details>`「看細項」：燈號句（含時間戳）＋模型分行表（輪數／輸入／讀到快取／命中率／輸出／花費，使用中的模型帶徽章，保溫自成一列且淡化）＋總計列＋診斷次數 chip＋牌價註腳。
  - **燈號純規則**（`DIAG_KEYS`，對照 usage_log.rs 那張表）：ok／ping 綠、warmup／expired 黃、prefix-broken／cache-skipped／no-cache／drop-lane 紅、single 灰（單發不打分）。原因代碼另一張 `REASON_KEYS`（九個）。10 語系各 50 鍵。
- 舊 log 遺留：資料目錄裡的 `prompt-cache.log`（純文字舊格式）不再寫入，包 6 只讀 `.jsonl`。
- **真桌驗收第一輪＋根因鎖死＋`cache-skipped` 標籤**（2026-08-04 凌晨，主線直做）：兩場四輪 hit_rate 0／62.3／0／0 與 0／0／0／58.8，取證與根因見 Current state。診斷規則補第八個標籤 `cache-skipped`——續聊輪、上輪送過內容（expected_cached>0）、讀寫皆 0＝CLI 這句沒帶標記，與整條路不支援的 `no-cache` 分開（usage_log.rs，判定加一個分支＋測試 2 條，commit 0b4ca49）。

## Verification

### 包 6 待驗收（2026-08-04 未做，另開對話進行）
程式面已綠：`cargo test` **217 passed**（新增 usage_report 2 條：一桌一份／ping 分列／agy 只算輪數／桌名對回；全桌總計＋線事件只進診斷）、clippy 無新增警告、`npm run build`（含 tsc）＋`check:i18n` 綠。**以下都要真的開 app 才算數**：
1. **分頁畫得出來**：設定 → 額度，預設落在當前桌，切桌與「所有桌總計」都能換。空桌顯示「還沒有紀錄」而不是空表。收合狀態要真的只有三件事（下拉、數字行、長條圖），版面看起來舒服；「看細項」點開才出現表格。
2. **數字對得上**：拿同一份 `文件/TableTavern/prompt-cache.jsonl` 自己加總，比對分頁的輪數／輸入／讀到快取／命中率／花費（尤其總計列與 ping 列不重複計算）。
3. **桌欄位真的有寫進去**：加欄之後跑一輪，log 新行要有 `"world":"<桌 id>"`；分頁不再把新紀錄丟進「未標桌」。
4. **使用中徽章**：切換檔位模型或傳輸層後，徽章跟著換到對的那一行。
5. **燈號句**：至少看過一次 warmup（換幕後第一輪）與 ok；文字是否白話、原因括號是否讀得懂。
6. **grok 金額**：跑一次 grok 呼叫，該行要有 `cost_usd`（0.2.111 實測有 `total_cost_usd`；沒有就是 CLI 版本變了，照慣例顯示「—」不會壞）。
7. **開桌生成認領**：用「一句話開桌」生一張新桌，那三次左右的生成呼叫要出現在新桌的統計裡（不是「開桌生成」那格）。
8. **agy 那一列**：跑一次 agy 呼叫，分頁要出現「輪數＋這個來源不回報用量」整列橫跨。
9. **十語系畫面**：至少抽查兩三個語系，看欄位標題有沒有撐爆表格寬度。

- **Opus 四輪真桌驗收**（2026-08-04 01:51–01:58，`文件/TableTavern/prompt-cache.jsonl` 第 16–20 行，lane `gm:claude-opus-4-8`）：

  | 輪 | diag | hit_rate | cached／expected | cost |
  |---|---|---|---|---|
  | 換場摘要 | single | — | 全額 11,290 | $0.131 |
  | 1 | warmup(first-turn) | — | 建 15,323 | $0.138 |
  | 2 | ok | 85.3% | 15,323／15,325 | $0.063 |
  | 3 | ok | 87.2% | 17,971／17,973 | $0.065 |
  | 4 | ok | 88.3% | 20,614／20,616 | $0.071 |

  每輪只差 2 個 token＝上輪送過的全中；命中率隨歷史變長遞增。四輪總額 $0.337 與 Sonnet 壞快取場的 $0.343 幾乎相同，但該場輸出量多約四成，故「Opus 太貴」的結論待 CLI 修好後重估，不在本任務處理。
- 包 7 保溫 ping：`cargo test` **214 passed**（含新 e2e：ping 後 session 檔逐字還原、下一輪續聊回覆編號仍為 2）、clippy 無新增警告、`npm run build`（含 tsc）＋`check:i18n`＋`npm test` 全綠。**前端計時器與離開提醒尚未實機跑過**（要真的等 12 分鐘＋真 CLI），屬真桌驗收項。
- `cargo test`：**213 passed; 0 failed**、cargo 零警告（2026-08-04 本機真跑，含包 5 新 2 條、改寫 2 條）；`npm run build`＋`check:i18n` 全綠。
- 包 4／包 5 未再花使用者額度做實機呼叫（機制由假 CLI e2e 覆蓋）；「模型是否乖乖輸出下一位行」屬真桌驗收項，看 `文件/TableTavern/prompt-cache.jsonl` 與推進行為即可確認。
- 案 C 改寫-resume 機制為實機查證（見上，scratchpad/rewrite-probe/probe.sh 可重跑；實驗腳本需 `env -u ANTHROPIC_BASE_URL -u ANTHROPIC_AUTH_TOKEN`）。
- CLI 用量欄位為實機冒煙查證，非文件推測：claude 同段 system prompt 連跑兩次，第一次 cache_creation=4771／cache_read=0（$0.0179），第二次 cache_creation=0／cache_read=4771（$0.0015）——快取在 CLI 端本來就自動運作。
- 2026-08-04 根因取證：CLI session 檔（`~/.claude/projects/-Users-pachelo-Library-Application-Support-TableTavern-cli-workspace/*.jsonl`）逐筆 usage 為據；官方單價反推 cost 與 usage 逐筆吻合（第 3 輪全裸 22,395 tokens 分毫不差＝usage 可信）；乾淨環境同旗標三連發（`env -u` 清認證）寫入 0→138→0 隔輪消失。cargo test 213 綠（含 cache-skipped 新斷言）。

### 已知限制
- ~~角色檔位混用會打散快取~~ **已解**（2026-08-03 深夜拍板＋實作）：lane 改按「線種:實際模型」分池，同模型角色共用、跨模型各自一條（便宜檔沒快取無所謂）；GM 獨立不硬合，理由見目標架構。
- ~~改卡／改世界書／改玩家卡＝重開線全量~~ **已解**（包 3）：快取存活走補丁、過期走追平，皆不重開。
- 收回上一句＝指紋不合＝重開線（正確），undo 截尾優化（truncate_from 已備好）排後續包。
- session 檔的 queue-operation／last-prompt 雜項行留有整包 prompt 副本（含機密段）：那些行不進模型上下文，私設隔離（模型可見面）不受影響；磁碟上正典檔本來就有這些資料。
- **claude CLI 2.1.210 resume 隔輪不帶快取標記**（官方問題，app 端無旗標可解；唯一相關旗標 `--exclude-dynamic-system-prompt-sections` 在自帶 `--system-prompt` 時明文忽略）＝命中率天花板被壓到「約一半輪次全額計費」，升級 CLI 前 66% 目標無法達成。

## Remaining / Next action
**程式面包 1–7 全部完成**（2026-08-04）。架構已通過 Opus 四輪真桌驗收；Sonnet 命中率受 CLI 官方 bug 壓制（2.1.220 仍在，取證見 Current state），app 端無事可做，等官方修。

**下一步＝實機驗收**，兩份清單：包 6 額度分頁八項（見 Verification「包 6 待驗收」）＋包 7 前端計時器與離開提醒（要真的等 12 分鐘＋真 CLI）。兩者可同一場真桌一起驗——保溫的花費會直接出現在額度分頁的 ping 列。驗完再收尾本任務。

### 目標架構（claude lane 專屬；其他 CLI 維持現行單發 flatten）
- **每桌按「線種:實際模型」分線**（2026-08-03 深夜拍板）：`chars:<model>`（解析到同一模型的角色共用一條；看解析後真正傳給 CLI 的模型字串，不看檔位——高中檔都覆寫成 sonnet 就同一條）＋`gm:<model>`（GM 獨立）。lane 概念取代「每角色一線」。GM 不與同模型角色硬合線：GM 凍結 system 多了 world.md／私設／GM 條目（憲法不能給角色看），硬合就得每輪注入抹掉、整包重付（真桌約一萬 token 級），比獨立線更貴；「貴模型只有 GM 用」的桌，GM 線本來就獨享快取。
- **凍結 system**（逐字不變，E7 證明動一字全滅）：
  - chars 線＝中性扮演指示（「你是這桌的扮演引擎，每輪告知你演誰」＋語言規範）＋全部公開角色卡＋玩家卡＋Public constant 條目——快照版。
  - gm 線＝現行 GM 指示＋world.md＋全 constant＋全卡（含私設）——快照版。
- **每輪只送新訊息**（`-p --resume <id> --system-prompt <凍結版>`，拿掉 `--no-session-persistence`）：上次水位之後的新事件＋設定補丁（若有）＋keyword 條目＋〔chars〕「現在你是X」＋X 私設＋X 限定可見條目；〔gm〕狀態欄＋導演指示。
- **回合後改寫 session 檔（chars 線，案 C 核心）**：(1) 上輪注入的私設／限定條目段抹掉（防洩漏給下一個被點的角色）；(2) 上輪 assistant 內容補「X：」名字前綴（模型輸出不帶前綴、扮演自然；session 歷史裡補標，下個角色才知道那句是誰說的；顯示層照舊）。改寫落在尾端，滅的快取塊只有幾百 token。
- **快照追平**：距上輪呼叫 >5 分鐘（快取已死）或重開 session 時，把現版素材併回凍結快照，零成本。
- **undo（收回上一句）**＝session 檔截尾到對應事件（快取匹配到截點，幾乎免費）；**換場**＝重開 session。
- **降級鏈（永遠可用）**：session 檔結構認不得／改寫後驗證失敗／resume 呼叫失敗 → 丟棄 session、重開全量重建（即現行 flatten 全文當首輪 prompt）。每次改寫用原子寫（暫存檔＋rename）＋寫後回讀驗證。

### session 檔偵察結果（實驗遺留真檔可對照：`~/.claude/projects/-private-tmp-…-cache-probe-ws/*.jsonl`）
- 路徑：`~/.claude/projects/<munged-cwd>/<session-id>.jsonl`，cwd＝`cli_workspace(app)`，munge 規則＝路徑非英數字元轉 `-`。
- JSONL 一行一物件：`type` 為 `user`／`assistant`（帶 `uuid`＋`parentUuid` 鏈、`message.role`＋`message.content`——user 為字串、assistant 為分段陣列）；雜項行（`queue-operation`／`ai-title`／`last-prompt`／`mode`）原樣保留不動。截尾＝刪葉端 user/assistant 行，uuid 鏈自然完整。

### 資料結構（app 資料目錄 per world，如 `lanes.json`；屬本機狀態、不進 .ai/）
每 lane 記：`session_id`、`sent_events` 水位（正典 transcript 的已送出位置）＋`sent_hash`、`snapshot`（凍結素材全文）＋`applied`（含歷來補丁的已傳達版本）、`pending_rewrite`（上輪注入段，供回合後抹寫）、`expected_reply`、`last_call_epoch`（追平判斷用）、`last_prompt_tokens`（下輪的理論可中量，診斷用）。正典與 session 對齊靠水位；水位對不上（外部改動）＝重開。

### 實作拆包順序（外派照 model-dispatch.md）
1. ~~包 1 session 檔操作模組~~ **完成**（2026-08-03，見 Completed；規格存主線 scratchpad/pkg1-spec.md 已隨 session 失效，內容即 session_file.rs 本身）。
2. ~~包 2 resume 流程地基~~ **完成**（2026-08-03 深夜，見 Completed）。
3. ~~包 3 凍結快照＋補丁＋追平＋原因代碼~~ **完成**（2026-08-03，見 Completed）。
4. ~~包 4 log 升級~~ **完成**（2026-08-04，見 Completed；標籤清單與欄位以 `usage_log.rs` 模組頂註解為準）。
5. ~~包 5 GM 旁白＋點名合併成一次呼叫~~ **完成**（2026-08-04，見 Completed）。
6. ~~包 6 額度分頁 UI~~ **實作完成、待實機驗收**（2026-08-04，見 Completed 與 Verification）。
   - **各來源現況**（2026-08-04 機上煙霧實測，app 原樣參數）：claude＋grok 有官方金額；codex 只回 token（訂閱制無逐筆計價，`--help` 無相關旗標）；agy 連 token 都不吐＝該行顯示「輪數＋不回報用量」。
   - **OpenRouter（API 路）**：官方可回報實際扣款，本包不接，claude 驗完後另補。
7. ~~保溫 ping~~ **完成**（2026-08-04，見 Completed；使用者拍板先做包 7，好讓包 6 的額度分頁一併驗到保溫效果）。
8. ~~agy 量測~~ **拍板不做**（2026-08-04：無頭輸出與 debug log 實測皆無用量資料，SDK wrapper 屬過度工程；日後官方 gemini-cli 上線再接原生統計）。

### 換幕提醒改標準（2026-08-04 實測算出＋拍板，已實作字數這條）
快取上線後成本結構翻轉：換一次幕＝摘要呼叫全額（$0.131）＋換幕後 warmup 全額重建（$0.138）≈ 連跑四輪的錢，而不換幕每輪只要 $0.065（舊內容按一折計）。原提醒門檻 8000 字元約在第 2–3 輪就跳，正是換幕最虧的時候。**換幕的理由從「省額度」改成「紀錄長到模型顧不上前面」**，並拍板兩個觸發標準：
1. **字數 30,000**（`SCENE_LENGTH_HINT_CHARS`，App.tsx:350-353）——已改，10 語系 `sceneTooLongHint` 文案理由同步換掉（npm build＋check:i18n 綠）。
2. **玩家離開太久＋紀錄夠長**——保溫 ping 連發**三次**（約 12 分鐘，一次上廁所倒水的長度）仍沒等到玩家推進，**且**該場紀錄超過 8000 字元（`SCENE_AWAY_HINT_MIN_CHARS`），才跳「間隔太久、快取已清除」的提醒（`sceneAwayHint`，另一份文案）。紀錄還短時重建本來就便宜，換幕反而白花一次摘要錢；保溫仍照樣停在三次（那是省錢邏輯，與提不提醒無關）。已隨包 7 實作。

### 驗收（整包完成後）
真桌實測看 `文件/TableTavern/prompt-cache.jsonl`：連續數輪 hit_rate ≥66%（目標「只有最後一句沒中」＝90%+）；中途改世界書／改卡當輪照樣命中；收回上一句後下一輪照樣命中；改寫失敗時自動降級重建、聊天不中斷。

## Constraints
同 tasks 檔。另：私設隔離憲法規則（transport.rs 頂部）在 chars 共用線靠「注入→回合後抹寫」維持——使用者已明示接受此實作（案 C，2026-08-03）＋接受「演出內容隱含私設影子」的殘餘洩漏。實驗腳本跑 claude CLI 必須 `env -u ANTHROPIC_BASE_URL -u ANTHROPIC_AUTH_TOKEN`（本會話環境會蓋掉訂閱登入、401）。
