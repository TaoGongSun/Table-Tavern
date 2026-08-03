# Handoff: prompt-cache-optimization

## Current state
A（穩定前綴重構）、C（命中率量測）、B（Claude 顯式斷點）＋D（CLI 路徑量測）全部實作完成；resume 續聊架構**包 1（session 檔操作模組）＋包 2（resume 流程地基）＋包 3（凍結快照補丁＋追平＋原因代碼）＋包 4（log 升級 JSONL＋診斷標籤）＋包 5（GM 旁白＋點名合併）完成**，cargo test 213 全綠、零警告（2026-08-04 本機）。claude 訂閱模式的角色對話與 GM 呼叫已全部走 resume 續聊線，改卡／改世界書當輪不再重開線；GM 每輪推進由兩次 CLI 呼叫（旁白＋點名）併成一次。下一步真桌驗收。

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
- 舊 log 遺留：資料目錄裡的 `prompt-cache.log`（純文字舊格式）不再寫入，包 6 只讀 `.jsonl`。

## Verification
- `cargo test`：**213 passed; 0 failed**、cargo 零警告（2026-08-04 本機真跑，含包 5 新 2 條、改寫 2 條）；`npm run build`＋`check:i18n` 全綠。
- 包 4／包 5 未再花使用者額度做實機呼叫（機制由假 CLI e2e 覆蓋）；「模型是否乖乖輸出下一位行」屬真桌驗收項，看 `文件/TableTavern/prompt-cache.jsonl` 與推進行為即可確認。
- 案 C 改寫-resume 機制為實機查證（見上，scratchpad/rewrite-probe/probe.sh 可重跑；實驗腳本需 `env -u ANTHROPIC_BASE_URL -u ANTHROPIC_AUTH_TOKEN`）。
- CLI 用量欄位為實機冒煙查證，非文件推測：claude 同段 system prompt 連跑兩次，第一次 cache_creation=4771／cache_read=0（$0.0179），第二次 cache_creation=0／cache_read=4771（$0.0015）——快取在 CLI 端本來就自動運作。

### 已知限制
- ~~角色檔位混用會打散快取~~ **已解**（2026-08-03 深夜拍板＋實作）：lane 改按「線種:實際模型」分池，同模型角色共用、跨模型各自一條（便宜檔沒快取無所謂）；GM 獨立不硬合，理由見目標架構。
- ~~改卡／改世界書／改玩家卡＝重開線全量~~ **已解**（包 3）：快取存活走補丁、過期走追平，皆不重開。
- 收回上一句＝指紋不合＝重開線（正確），undo 截尾優化（truncate_from 已備好）排後續包。
- session 檔的 queue-operation／last-prompt 雜項行留有整包 prompt 副本（含機密段）：那些行不進模型上下文，私設隔離（模型可見面）不受影響；磁碟上正典檔本來就有這些資料。

## Remaining / Next action
**2026-08-03 晚全部拍板完畢（案 C＋只做 claude，其他 CLI 日後分別實測）**。程式面包 1–5 全部完成，下一步：**真桌驗收**（見下方驗收段），通過後補包 6 額度分頁 UI。以下為定稿規格。

**2026-08-03 追加拍板（額度分頁需求）**：使用者要額度計量分頁顯示命中率＋顏色燈號＋壞狀況原因句，且原因由 app 純規則判定（不靠 AI）。影響：(1) 包 3 順手把**線重開／降級的原因代碼**落 log（重開當下記，事後推不回來）——已隨包 3 完成；(2) 包 4 log 改**結構化 JSONL**＋診斷標籤定死成有限清單（正常／前綴斷於第幾段／過期／重開-原因X／已降級），資料齊到 UI 拿來就能畫；(3) **包 6 額度分頁 UI** 新增，排在真桌驗收**之後**再補。

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
6. **包 6 額度分頁 UI**（2026-08-03 拍板，排真桌驗收之後）：設定頁「額度花費＋命中率」分頁，讀 JSONL log 畫花費＋命中率＋顏色燈號＋原因句。
7. **保溫 ping**（已拍板可考慮，最後做）：距上輪近 4 分鐘且玩家還在（視窗聚焦／打字中）發迷你訊息刷新壽命；ping 後把垃圾訊息從 session 檔截掉（快取時鐘已被讀取刷新，截尾不影響）。細節實作時拍。
8. agy 量測（未拍板，擱置）。

### 驗收（整包完成後）
真桌實測看 `文件/TableTavern/prompt-cache.jsonl`：連續數輪 hit_rate ≥66%（目標「只有最後一句沒中」＝90%+）；中途改世界書／改卡當輪照樣命中；收回上一句後下一輪照樣命中；改寫失敗時自動降級重建、聊天不中斷。

## Constraints
同 tasks 檔。另：私設隔離憲法規則（transport.rs 頂部）在 chars 共用線靠「注入→回合後抹寫」維持——使用者已明示接受此實作（案 C，2026-08-03）＋接受「演出內容隱含私設影子」的殘餘洩漏。實驗腳本跑 claude CLI 必須 `env -u ANTHROPIC_BASE_URL -u ANTHROPIC_AUTH_TOKEN`（本會話環境會蓋掉訂閱登入、401）。
