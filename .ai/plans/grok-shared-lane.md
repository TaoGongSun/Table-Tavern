# grok-shared-lane — grok 角色改走共線

## 實驗（2026-10-06，grok 1.0.46，app 同組旗標＋GROK_CAMPAIGNS=0，測試通道 grok-home，grok-4.6）

session 目錄：`$GROK_HOME/sessions/<URL 編碼 cwd>/<session id>/`。內容相關的只有 `chat_history.jsonl`（送給 API 的歷史）與 `updates.jsonl`（續聊時用來重建／比對）；`system_prompt.txt` 是凍結 system。其餘（events、signals、usage、summary、rewind_points）不含正文。

1. **reasoning 會洩漏私設**：第一輪 reasoning 摘要明文寫出暗號；chat_history 的 `reasoning` 項目（summary＋encrypted_content）與 updates 的 `agent_thought_chunk` 都帶著。只抹 user 段、留 reasoning → `-r` 探測模型直接答出暗號。summary 為空時 updates 沒有 thought chunk，但 chat_history 仍有 encrypted reasoning，所以 reasoning 項目要每輪無條件拿掉。
2. **抹 user 段＋拿掉 reasoning＋補名字前綴**（兩檔都改）→ 探測答「沒有」；CLI 原樣保留改過的檔，開頭三行（system、user_info、技能清單）全程不變。
3. **快取**（每輪 cached／總輸入；128＝整段沒中）：

| 線 | 第 2–8 輪 |
|---|---|
| 不抹（對照） | 128、10624、10880、11008、128、4352、128 |
| 只拿 reasoning＋前綴 | 10368、10496、10624、10880、11008、11136、11264（全中） |
| 完整抹寫 | 128、10496、10624、10752、10752、128、128 |
| 只抹 user 段＋前綴 | 10496、10496、128、128、4352、11008、11008 |

命中輪只少最後一截增量（約 100–350 tokens，即上一輪 user 段＋回覆）；整段 128 在對照組同樣出現，屬 xAI 分流，交給 usage-cache-audit。

4. 大 prompt（41KB）、約 500 字回覆在 updates 都是單一 chunk；assistant 內容與串流文字逐字相同。
5. 整個 grok-home 抹寫後只剩 `sessions/<cwd>/prompt_history.jsonl`（CLI 輸入歷史，不進模型上下文）留有原文；`session_search.sqlite` 沒有。
6. 怪異 prompt 下模型會呼叫伺服器端 `x_search`、`send_feedback`（被 deny 擋），工具參數可能帶到私設；chat_history 出現 `backend_tool_call`／`tool_result`、updates 出現 `tool_call`／`tool_call_update`。
7. **鎖**：四個 `.lock`（chat_history、updates、summary、rewind_points）grok 都遵守。我方以 flock 或 fcntl 持任一把，`-r` 就一直等（>15 秒，放鎖後 3 秒內完成）。卡住時 lsof 顯示 grok 只開著正在等的那把，沒有同時持其他鎖——它不巢狀持鎖，所以我方限時嘗試不會互鎖。
8. **壓縮**：headless `-p "/compact"` 可手動觸發。觸發後 chat_history 換成 `synthetic_reason: compaction_meta` 的摘要項；updates 多 `compaction_checkpoint`、`auto_compact_completed`；signals `compactionCount` 變 1；新增 `compaction/`（INDEX.md、segment_*.md 含摘要與最後回覆節錄）、`compaction_checkpoints/*.json`、`compaction_requests/*.json`（壓縮前完整 chat_history）。私設若在壓縮當下還在歷史裡，就會被寫進這些檔與摘要，下次 `-r` 回到上下文。自動壓縮門檻是 context 的 85%（grok-4.6 約 217k tokens），只能用 config.toml `[session]` 調，GROK_CONFIG 疊加層不收 `session`。
9. **session 目錄外的檔不進上下文**：在 summary.json 的 `session_summary`／`generated_title`／`last_turn_summary`／`session_recap`、`prompt_history.jsonl`、`session_search.sqlite` 的 `session_docs` title／content 各植入不同標記，`-r` 探測模型答「沒有」，chat_history 也沒帶入。自動標題會改述私設（例：「Roleplay fox with secret cipher」），屬落地資料。

## 設計

### 線名與私設
- grok chars 線改回 `chars:<模型>`（全角色共用），GM 仍 `gm:<模型>`；Agy 維持一角一線＋hoist。
- 私設與 claude 同一套：`chars_lane_turn(hoist_private=false)`，私設＋限定條目＋角色狀態放在 tail 的機密段 `confidential`，`prefix`＝「X：」。`chat.rs` 的 `hoist` 只對 Agy 成立。凍結 system＝`chars_lane_system` 原樣，不再含私設，所以只改私設不會重開。
- 漂移規則沿用 `plan_turn` 的 grok 分支（`applied != frozen_system` → `SystemChanged` 重開）；保溫照舊不 ping grok。
- `run_turn` 防呆改成「Agy 帶機密段或前綴才擋」。
- 舊 `chars:<模型>:<角色 id>` 鍵不相容、不清理，殘留在 lanes.json 無害〔模型判斷·未裁決〕。

### 並行（同桌 lane 呼叫串行）
- 現況：permit 是共用讀鎖，`register_turn` 不擋第二輪；後端只在 GM 生成中擋角色呼叫。UI 的 `busyRef` 擋住 GM＋角色、角色＋角色，但保溫 ping 能與回合同時跑；多視窗或直接 invoke 也繞得過。
- `lanes` 加每桌一把 async mutex（lanes.json 路徑 → `Arc<tokio::Mutex>`）。`run_turn` 從讀 store 到最終寫 store、含 CLI 呼叫與抹寫全程持有；`keepalive` 也全程持有同一把（它整份讀寫 lanes.json、還會截 session 檔，只鎖 run_turn 仍會把撤銷的 session id 寫回）。claude／agy 一起受惠。

### 回合後抹寫（新檔 `lanes/grok_session.rs`，只對 grok chars 線；GM 線完全不動）
- **定位**：`$GROK_HOME/sessions/*/<id>/` 恰好一個目錄（群組名可能是 URL 編碼或 slug＋hash，不自己重算）。`LaneCall` 加 `grok_home`。
- **鎖**：依序對 `chat_history.jsonl.lock`、`updates.jsonl.lock` 用 std `File::try_lock` 輪詢，限時 2 秒，拿不到就丟線。CLI 此時已退出，有 mutex 保證同線沒有別的 grok 行程。
- **壓縮跡象先擋**：signals `compactionCount > 0`、`compaction/`／`compaction_checkpoints/`／`compaction_requests/` 任一存在、chat_history 有 `compaction_meta` 項、updates 有 `compaction_checkpoint`／`auto_compact_*` → 丟線（原因 `compacted`）。
- **chat_history**：最後一則真 user（帶 `prompt_index`）之後只允許 `reasoning` 與恰好一則 `assistant`（字串內容、無 `tool_calls`）；`backend_tool_call`、`tool_result` 或任何其他型別都丟線。本輪有機密段時，該 user 的 text 須恰好含一次，刪掉；沒有機密段就不找、不因 0 命中丟線。刪掉所有 `reasoning`（含 summary 為空、只有 encrypted_content 的），assistant 補前綴。
- **updates**：最後一個 `user_message_chunk` 之後只允許 `agent_thought_chunk`、`agent_message_chunk`（≥1）、`turn_completed`、`background_tasks`（內容仍交殘留掃描）；`tool_call`、`tool_call_update`、`hook_execution`、`retry_state` 或未知型別都丟線。機密段規則同上；刪 thought chunk，第一個 message chunk 補前綴。
- **跨檔一致**：去機密後 chat_history 的 user text（剝 `<user_query>\n…\n</user_query>`）＝updates 的 user chunk；message chunk 拼接＝assistant 內容＝前綴＋本輪回覆；前綴只出現一次且在開頭。任一不符丟線。模型模仿歷史格式自己先寫了「名字：」時不重補（同 claude 的 `prefix_last_assistant`；實機約三分之一的角色台詞會這樣）。
- **寫回**：沒動的行保留原字串，改過的行用 `serde_json` 緊湊序列化（鍵序重排實測可續聊）。暫存檔放 `$GROK_HOME/tt-rewrite-tmp/<session id>-<檔名>-<pid>`（同一磁碟、不在 session 目錄；不同桌各一把 mutex，檔名帶 id 才不互撞），寫完 fsync、`std::fs::rename` 蓋回、回讀比對。Windows 的 `rename` 是 MoveFileExW＋REPLACE_EXISTING、可覆蓋既有檔（同 updater/store.rs、data/config.rs 的既有結論），未實測。先寫 chat_history 再寫 updates；兩次 rename 不是共同交易，所以 `pending_rewrite` 只在兩檔都寫完、殘留掃描通過、store 落檔時才清，中途失敗或崩潰下輪必定 `PendingRewrite` 重開。
- **殘留掃描**：只在這輪送了非空機密段時做（空字串會命中每個檔）。走遍 session 目錄（含子目錄）。`.jsonl` 逐行、`.json` 整檔先解碼，再遍歷所有字串值；其他檔當 UTF-8 文字。任一處含機密段全文就丟線；JSON 解析、目錄遍歷、讀檔任一失敗都算掃描失敗、丟線，不跳過判乾淨。
- **丟線**：先從 store 移除該線並落檔，再刪 `sessions/<群組>/<這個 id>` 目錄。找到 0 個目錄當已刪；找到多個一個都不刪、只丟線並記帳。不准刪上一層。刪除失敗只記 `cleanup-failed`，store 已沒有這個 id，不會再續聊。帳本記丟線原因（`rewrite-failed`／`compacted`／`lock-timeout`）。
- **撤銷舊線**（grok）：下列情況都走「store 移除→落檔→刪目錄」：中止、CLI 開線失敗、續聊失敗後降級重開之前、重開時 store 裡還有舊 grok 線（含崩潰留下的 `pending_rewrite`）。store 落檔失敗時回錯，不續用舊 id（磁碟上的 pending 仍在，下輪必重開）。
- **已知**：回覆本身含「名字：」時，「前綴只出現一次」的檢查會丟線，只損快取。
- cached 整段 128 只是 xAI 分流，照常續聊，不算失敗、不重開。

### 不處理
- `prompt_history.jsonl`、`session_search.sqlite`、summary.json 標題：在 session 目錄外或不進 `-r` 上下文（實驗 9），屬落地資料〔模型判斷·未裁決〕。
- 自動壓縮門檻：不寫 config.toml（沿用 grok-system-override 只用環境變數的決定）。壓縮就丟線重開；單幕逼近約 217k tokens 時每輪都會重開，交給 long-prompt-scene-hint 的換幕提醒。

## 驗收
- **cargo（fixture 照實驗檔形手寫，不含憑證）**：
  - 抹寫成功：有機密段、沒私設的角色輪不丟線、多 chunk、JSON 跳脫（換行／引號）、空摘要＋encrypted reasoning。
  - 丟線：0／2 處命中、`backend_tool_call`、`tool_call`、`retry_state`、壓縮跡象、跨檔不一致、殘留（解碼後才看得到的跳脫字串）、鎖逾時。
  - 刪除：兩個同 id 目錄時一個都不刪。
  - 注入：第一檔成功第二檔失敗、store 寫失敗 → 下輪 `PendingRewrite`。
  - GM 線檔案不動；Agy 仍擋；grok 線名無 scope；只改私設不重開。
  - 受控競態：GM＋角色、角色＋角色同桌並發，store 不互蓋、同一 session 不被兩輪同時 resume。
  - 保溫競態：保溫已讀 store、ping 進行中 → 角色輪重開落檔 → 保溫收尾，舊 session id 不復活。
  - 沒送機密段不做殘留掃描；掃描遇到壞 JSON 判失敗；`background_tasks` 放行；暫存檔名帶 session id。
- `npm run verify` 全綠。
- **測試通道**（迷霧酒館、grok-4.6，兩個有私設的角色輪流 ≥6 輪）：
  - lanes.json 只有一條 grok chars 線；每輪後 session 目錄解碼掃描找不到任何私設、沒有 reasoning。
  - 換角色不重開；改卡重開一次；只改私設不重開；中途停止後下輪重開；GM 仍拿得到私設。
  - cached 整段 128 不觸發重開。
  - 觀察跨角色連續劇情的品質（角色不混用彼此私設、接得上前文）。
- **未驗**：Windows 的鎖 handle 與刪目錄。

## 驗收結果（2026-10-07）
- cargo：grok_session 12 項、lane 端到端 6 項（共線無殘留、GM 不動、形狀不符撤銷、store 落檔失敗留 pending、續聊失敗撤銷、同桌並發串行）、保溫競態 1 項；拿掉每桌鎖時並發與保溫兩項轉紅。`npm run verify` 10 步全綠（vitest 1034、cargo 1066）。
- 測試通道（迷霧酒館、grok-4.6、狐狸 FOX-7731／騎士 KNT-4402 輪流 20 輪以上）：
  - 一條 `chars:grok-4.6`；每輪後 session 目錄解碼掃描私設 0 筆、reasoning 0、thought 0；凍結 system 不含私設。
  - 換角色不重開；只改私設不重開；狐狸上場（凍結 system 變動）重開一次（`reopen: system-changed`）後續聊；按停止後撤銷（`aborted`、目錄刪除），下輪 first-turn 重開；app 重啟後照常續聊。
  - 命中輪 84–94%（每輪少約 1300 tokens＝上一輪 user 段＋回覆）；整段 128 約占四成，不觸發重開。
  - GM 線 system 含狐狸私設、reasoning 保留、無前綴。
  - 角色台詞沒有出現別人的私設。
- 實測中發現並修正：模型自帶「名字：」前綴原本會被判成丟線。
- 未驗：Windows 的鎖 handle、刪目錄與 rename 覆蓋。
