# grok-backend-tools — 做法

## 查證結論（grok 1.0.46）
- 伺服器端工具＝`features.backend_tools`（預設 true，環境變數 `GROK_BACKEND_SEARCH`；user-guide/26-config-reference.md）。`--disable-web-search` 只拿掉客戶端 web_search，管不到它；`models_cache.json` 四個模型都標 `supports_backend_search: true`。
- `send_feedback` 是客戶端工具（binary 內 `xai-grok-tools/.../grok_build/send_feedback.rs`），已在 `GROK_CHAT_DISALLOWED_TOOLS`，不屬本案根因。
- 伺服器端呼叫不出現在 streaming-json 串流，只落在 session 的 chat_history.jsonl（現行 `rewrite_chat_history` 視為非預期項目→丟線）。
- 所有 grok 呼叫（聊天、續聊、生圖、目錄、登入／探針腳本）都走 `cli::grok_envs`，改一處全涵蓋。

## 實測（測試 root 的 grok-home，grok-4.5、app 同組旗標，誘導查 X 即時貼文的角色 prompt；腳本在 scratchpad/bt）
| 組 | 伺服器端呼叫 | input tokens |
|---|---|---|
| 現行 ×2 | 11、10（x_keyword_search／x_user_search／x_semantic_search） | 36879、20521 |
| `GROK_BACKEND_SEARCH=0` ×2 | 0、0 | 3710、3710 |
| `GROK_CONFIG` 加 `"features":{"backend_tools":false}` ×2 | 0、0 | —（其一快取命中，不作輸入量證據）、3838 |

## 做法
1. `grok_envs` 加 `GROK_BACKEND_SEARCH=0`；`GROK_SAMPLING_OVERLAY` 改名 `GROK_CONFIG_OVERLAY`，內容加 `"features":{"backend_tools":false}`。兩層都設的理由：一條走環境變數、一條走 overlay 白名單，CLI 升版弄丟其中一條還有另一條。requirements（`/etc/grok/requirements.toml`、MDM）把 `backend_tools` pin 成 true 時兩層一起失效（pin 壓過設定檔、環境變數與命令列），不在已證明範圍。
2. 生圖共用 `grok_envs`，一起關〔作者裁決 2026-10-07〕。
3. 丟線偵測維持現狀〔模型判斷·未裁決〕：它只保護後續共線（工具結果不留在線上）；萬一開關失效，呼叫當下參數已送出、該輪照樣撤線翻倍，不是防外洩的保險。
4. 測試：`request/tests.rs` 斷言新 env 與 overlay，並續斷言 temperature／top_p／`GROK_CAMPAIGNS`；`cli_setup.rs` 腳本字串斷言跟著更新。

## 驗收
- 重現腳本兩層同設，開線 `-s` 後續聊 `-r` 至少一輪：同一 SID 續用；chat_history 的 `backend_tool_call` 與 updates 的 `tool_call` 都是 0；無丟線；續輪只送新增輸入。關工具後模型把工具語法寫進正文屬正常字串。
- 測試通道真桌普通角色回合：同一條 lane 續聊不撤線，並對該 session 數上述兩種紀錄。
- 生圖實送一次，產物存在且可讀。

## 驗收結果（2026-10-07，verify 10 步綠；Sol、Grok 驗收通過）
- 重現腳本兩層同設，同一 SID 開線＋續聊：laneA 開線與續聊都用誘導搜尋 prompt（續聊那輪退化，見下方觀察，手動中止；計數仍 0）；laneB 開線用誘導 prompt、續聊用一般台詞。兩組 chat_history `backend_tool_call` 0、updates `tool_call` 0；舊設定同一誘導 prompt 跑兩次，`backend_tool_call` 為 11 與 10（第一次的 updates `tool_call` 也是 11，證明計數有效）。laneB chat_history 只有 user／reasoning／assistant，續輪未命中快取 687、快取 3712。
- 測試通道塞拉菲桌（grok-4.5）角色回合兩輪：新開 `chars:grok-4.5` 線後續聊同一 session、`sent_events` 43→44 不撤線；該 session `backend_tool_call` 0、`tool_call` 0；prompt 14575→14968（比上一輪多 393，未命中快取 504）。
- 生圖 `generate_character_image`（source=grok）實送一次：PNG 832×1248 落 gen-gallery、可讀；該 session `backend_tool_call` 0（客戶端 image_gen 等工具照舊）。

## 範圍外觀察〔模型判斷·未裁決〕
- 誘導搜尋 prompt 在關工具後，grok-4.5 會把 web_search 語法寫進正文；laneA 續聊那輪退化成連續空白亂碼、6 分鐘不停。持續吐字不算串流停滯，app 目前不會攔。一般角色回合沒遇到。
