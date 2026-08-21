# api-shared-lane — API 路徑改走 chars 共線

**狀態：設計已拍板（2026-08-21 與 Sol 收斂）。等使用者同意即可開工。**

## 問題

claude 路徑早已做過「全角色共用一條線」（prompt-cache-optimization 包 2），API／codex／grok 路徑沒有——被 [lib.rs:2201](../../src-tauri/src/lib.rs) 一行 `if chat_transport(&config) == "claude"` 擋在外面：

```rust
if chat_transport(&config) == "claude" {
    let frozen = transport::chars_lane_system(&cards, ...);  // 全部角色卡，共用凍結 system
    return lanes::run_turn(...);
}
let messages = transport::assemble_messages(&card, ...);      // 單一角色，system 是該角色專屬
```

兩份 system 從第一個字就不同：`chars_lane_system` 是「你是這場多人桌上角色扮演的**扮演引擎**」＋全部公開角色卡；`assemble_messages` 是「你正在扮演**「加爾」**」＋加爾自己的公開與私有設定。前綴快取逐字比對，於是 API 路徑**每換一個角色就整包重算**。

## 實測證據（2026-08-21，OpenRouter `stealth/ox-alpha`）

| 時間 | 間隔 | 輸入 | 讀到快取 | 命中率 | 這輪是誰 |
|---|---|---|---|---|---|
| 14:55:32 | — | 6,581 | 64 | 1.0% | 角色 |
| 15:16:41 | 21 分 | 9,235 | 64 | 0.7% | GM |
| 15:18:49 | 2 分 | 7,720 | 64 | 0.8% | 加爾 |
| 15:21:55 | 3 分 | 8,174 | **7,680** | **94.0%** | 加爾（同角色連續） |

同角色連續能到 94%，其餘一律掉到 64——那 64 是 provider 的固定系統前綴，不是玩家的內容，**掉到 64 等於全滅**。另在測試前綴上量到 98.7%，證明「送一次同樣的請求」確實能刷新快取。

## 障礙：transcript 的 role 分配也是角色專屬的

[transport.rs](../../src-tauri/src/transport.rs) 把歷史事件轉成 messages 時：

```rust
TranscriptKind::Dialogue if event.speaker_id == card.id => ("assistant", …)   // 自己說的
TranscriptKind::Dialogue => ("user", format!("{}：{}", 名字, 內容))            // 別人說的
```

同一則對白，對加爾是 `assistant`、對雷恩是帶前綴的 `user`。**光換 system 不夠**，整段歷史的形狀仍因角色而異。

claude lane 的解法（見 lanes.rs 模組頂註解）：session 裡所有角色台詞一律 `assistant`，回合後補「X：」名字前綴讓下一個角色知道那句是誰說的（`run_turn` 的 `prefix` 參數）。API 路徑要共線就得跟著換這條轉換規則。

## API 無狀態的優勢：私設抹除機制不用做

`chars_lane_turn` 回傳的 `LaneTurn.confidential` 是給 claude lane 在回合後把注入的私設從 session 檔抹掉用的（原子寫＋回讀驗證那一整套）。API 路徑每輪從正典 transcript 重新組裝，上一輪注入的私設不會出現在下一輪的 messages 裡——**天然乾淨，這段完全不用實作**。

## 數字（以健身兔子那桌為準）

角色卡合計約 1,810 tokens，單張約 280。

| | system 大小 | 換角色時 |
|---|---|---|
| 現在 | 約 280 | 全額重建（實測 7,720） |
| 共線後 | 約 1,810 | 命中 95%+，只付新增部分 |

每次呼叫輸入多約 1,500 tokens（+19%），換來換角色不再從頭算；四角色輪流的節奏下第二輪就回本。

## Anthropic 顯式斷點會被共線打壞（2026-08-21 查證）

[transport.rs:1947](../../src-tauri/src/transport.rs) 的 `anthropic_messages` 用 role 猜穩定性：

```rust
let last_assistant = messages.iter().rposition(|m| m.role == "assistant");
if message.role == "system" || Some(index) == last_assistant { /* 掛 cache_control */ }
```

現行 assistant／user 交錯，「最後一則 assistant」是已定案不再變的台詞，逐輪增量命中成立。共線後兩個候選都會壞：

- **全 assistant**：台詞相鄰同 role，`push_merged`（[transport.rs:110](../../src-tauri/src/transport.rs)，只合併相鄰同 role）併成一個每輪尾端追加的巨型 block，斷點掛在它上面每輪失效。
- **全 user**：`last_assistant` 是 `None`，只剩 system 一個斷點。

對 Anthropic 系的淨效果分兩種情境：**換角色** 0% → 23%（system 本來也是角色專屬、跟著全滅，現在穩定了，是改善）；**同角色連續** 從高命中掉到 23%（現行那則穩定 assistant 被併進巨型 block），是退化。

這條只影響走顯式斷點的 `anthropic/*`。實測那 94% 是 OpenRouter 對 `stealth/ox-alpha` 的自動前綴快取——供應商端逐字比對序列化後的 prompt，與 message block 邊界無關。

## CLI 三條是另一個形狀（2026-08-21 實測）

codex／agy／grok 走 `cli::flatten_messages`（[cli.rs:140](../../src-tauri/src/cli.rs)）把 messages 攤平成 `(system, prompt)` 兩個字串。攤平時 assistant 被補上 `assistant_label`＝`card.name`（[lib.rs:2281](../../src-tauri/src/lib.rs)），而別人的台詞在 `assemble_messages` 裡本來就帶名字——**攤平後每一行的文字與角色無關**（「加爾：加爾抬起頭。」在加爾那輪與雷恩那輪一字不差）。

臨時探針跑真的 `assemble_messages`＋`flatten_messages`（4 角色 7 事件），唯一差異是**空行位置**：`push_merged` 合併相鄰同 role，換角色時分組跟著變，`history.join("\n\n")` 的斷句就移位。共同前綴 25 字元／全長 97。

codex 15 筆實測印證這個形狀：

| 讀到快取 | 次數 |
|---|---|
| **9,984**（逐次一字不差） | 8 |
| 18,176／19,200／20,224 | 5 |
| 12,032／14,080 | 各 1 |

`9,984` 重複 8 次、跨越 08-04 與 08-21 兩批，是 codex CLI 自己的固定前綴（API 那條 `64` 的放大版）。我們送的那段約 9,200 tokens 要嘛全中（→90%+）、要嘛全滅（→掉回 9,984）——**換角色全滅、同角色連續全中**，與 API 路徑同形。共線可回收的就是那 8 筆 × 約 9,200 ≈ 74,000 tokens。

兩個推論：

1. **CLI 三條不受「角色混淆」風險影響**——role 在攤平時就消失，模型看不到 assistant／user 的區別。該風險是 API 路徑專屬。
2. **共線組裝器一次修好四條路**：全部台詞改 assistant 後，role 序列只跟事件種類有關，`push_merged` 分組變成角色無關，攤平文字自動逐字相同。下面「雙重前綴」那個洞的修法同時解掉斷句問題。

## agy 的用量拿得到了（原結論過期）

agy **1.1.8** 的 release note：JSON 與 stream-json 輸出的 usage 物件開始回報含 `cache_read_tokens` 的 token 帳。[cli.rs:4](../../src-tauri/src/cli.rs) 記的查證版本是 1.1.3，比這個功能還早；實機是 1.1.17。

不花額度實測（`-p "/usage"` 這類唯讀指令不起 turn、不耗額度）確認 envelope：

```json
"usage":{"input_tokens":0,"output_tokens":0,"thinking_tokens":0,"cache_read_tokens":0,"total_tokens":0}
```

`--output-format json` 會把整包壓到最後才吐、**串流消失**；要改就改 **`--output-format stream-json`**（NDJSON，最後一則 `{"event":"result","result":{…usage…}}`，串流與用量兼得）。這是 CLI 那條量得到快取的前提——現在 agy 23 輪全是 `unreported`。

**金額仍然拿不到**：codex 0.149.0-alpha.4 的 session 檔（417 KB、26 個 `token_count` 事件）裡 `cost`／`usd` 字樣 0 次，只有 `rate_limits` 與 `credits.balance`；agy 的 print-mode envelope 沒有金額欄位（`estimated_cost_usd` 只存在於內部 protobuf）。四支 CLI 報金額的仍只有 claude 與 grok。

## 拍板結論

1. **role 分配採「全部台詞皆 assistant ＋名字前綴」**，與 claude lane 同一條規則。assistant 在共線語意下代表「扮演引擎過去的產出」，每句帶名字時語意成立。「全部當 user」留作 fallback——它避開角色混淆，但失去模型自身輸出的示範錨、且戲內文字更容易被當成使用者指令，不當首選。由驗收決定是否啟用 fallback。**風險敞口只有 API 一條路**——CLI 三條的 role 在攤平時就消失，看不到 assistant／user 的區別。
2. **「只換 system、不動 role」不採**：對 API 路徑，第一則角色對白就是差異點，命中率上限只有 system 那段（該桌約 23%）。（對 CLI 三條，只換 system 其實接近滿分——歷史攤平後本來就角色無關；但統一組裝器順手就把 role 改了，不必為它們另立一種做法。）
3. **單角色桌不開第二條組裝路徑**，只在組裝函式內加一個條件：`load_active_cards` 回傳 1 張時，把該角色私設留在 system（只有一個角色，不會洩漏）。該函式（[lib.rs:2292](../../src-tauri/src/lib.rs)）已排除 `archived` 與 `auto_hidden`，玩家卡走 `read_player_card` 本就不在名單，條件零額外實作。
4. **共線先、保溫後**：換角色頻率遠高於發呆超時；保溫成本＝讀價×輸入，要等共線後的新 token 基線才估得準；`max_tokens:1` 會動到玩家自己的額度，不預設啟用。
5. **分成 A → B → C 三包**（2026-08-21 依實測資料重排，取代原「兩包」分法）：
   - **包 A｜agy stream-json（約 80 行）**：獨立先行，不綁進 B。它改的是 CLI 參數、串流解析、usage 落帳；B 改的是 prompt 語意——混在一起故障歸因會分不清。A 先完成，B 的四路驗收才不缺 agy。
   - **包 B｜共線組裝器（約 125 行）**：一支統一組裝器同時修好四條路。全部台詞改 assistant＋名字前綴後，role 序列只跟事件種類有關 → `push_merged` 分組變成角色無關 → CLI 攤平文字自動逐字相同，雙重前綴那個洞順帶解掉。
   - **包 C｜anthropic block（約 70 行）**：**降為條件觸發，不再是上線門檻**。帳本 638 筆裡 `api` 走過的模型只有 `stealth/ox-alpha` 與 `deepseek`，零筆 `anthropic/`；三條量過的路徑全是供應商自動前綴快取，不看顯式斷點。
6. **包 C 的兩條附帶條款**：未完成 C 前，`anthropic/*` 共線後**只保證 system 快取**，不能宣稱完整支援；若日後預設模型、使用者設定或發行對象開始包含 `anthropic/*`，**C 自動升回上線門檻**。C 的內容不變——歷史拆成「一正典事件一個 content block」（不可變、只追加），動態世界書、狀態、私設、「現在你是 X」留在斷點後方，**不動全域 `push_merged`**（它同時服務 GM 路徑），斷點改由組裝器明確交付「最後一個穩定 block」的位置，`anthropic_messages` 不再用 `role == "assistant"` 猜。
7. **包 B 的已知取捨：對沒有快取的模型是純增。** `deepseek-v4-pro-0813-free` 27 筆命中率恆 0（有回報、值就是 0），共線每輪多送約 1,500 tokens 零回收——該模型平均輸入 4,411，等於 **+34%**。自動退回機制另案處理：[no-cache-model-optout](no-cache-model-optout.md)。

各路徑的成本佔比（帳本實測平均輸入，動工前粗估即可，正式答案靠包 B 後的多輪實測）：

| 路徑 | 平均輸入 | 加 1,500 佔比 |
|---|---|---|
| `deepseek`（無快取） | 4,411 | +34.0% |
| api `stealth/ox-alpha` | 7,927 | +18.9% |
| codex | 21,308 | +7.0% |
| grok | 68,692 | +2.2% |
| agy | 待包 A 提供 | — |

### 三個實作前必修洞

- **`flatten_messages` 雙重前綴**：[cli.rs:154](../../src-tauri/src/cli.rs) 無條件 `format!("{assistant_label}：{}", content)`。共線後台詞已自帶「X：」，codex／grok 路徑會變成「加爾：雷恩：……」。
- **agy 輸出格式**：現在走預設 `text`（[cli.rs:464](../../src-tauri/src/cli.rs) 的 `agy_args` 只給 `-p prompt`），量不到快取。改 `--output-format stream-json` 並補一支 `parse_agy_usage`，順手把 [cli.rs:4](../../src-tauri/src/cli.rs) 的版本註記從 1.1.3 更新到實機版本。
- **尾端指定 block 不存在**：`cli_closing` 只用於 [lib.rs:1679](../../src-tauri/src/lib.rs) 的 CLI 攤平那條，API 路徑的「你是誰」寫死在 system。共線後「現在你是 X」必須成為 messages 尾端一則真正的 user block。

### 驗收

跨模型角色辨識三項，不得只憑 claude lane 放行：**錯認前言者**（把別人的台詞當自己說過）、**串角**（替其他角色代言）、**私設洩漏**（私設從 system 移到尾端後遵循度是否夠）。**只需在 API 路徑驗**——CLI 三條的 role 在攤平時就消失。

組裝層核心測試：**兩個不同角色組裝出的 messages，除最後一則外必須逐字相同**；CLI 側再加一條「攤平後的 `(system, prompt)` 逐字相同」。

**角色辨識三項的 CLI 側結果（2026-08-21 實跑）**：用真的組裝器產出一張三角色虛構桌的共線 prompt（灰狼加爾／花豹雷恩／棕熊布洛，各有好抓的私設關鍵字），指定加爾發言，送 codex `gpt-5.6-terra`、agy `gemini-3.1-pro-low`、grok `grok-4.6`（grok 跑在 app 的隔離環境裡，否則會載入使用者 `~/.claude` 的 hooks 與 CLAUDE.md 污染測試）。

- **串角**：沒有。只有加爾在動、在說話。
- **私設洩漏**：理想表現——玩家問「右手臂有印記的人」，加爾把右手往身側收、確認護腕遮住烙印，**用了**私設而沒說破。別人的私設靜態檢查也確認一個字都沒進他的上下文。
- **錯認前言者**：CLI 攤平後 role 消失，這項在它們身上結構上不存在，未驗。留給 API 路徑的實機驗收。

三家的表現都是「用了私設卻不說破」：玩家問「右手臂有印記的人」，加爾都是把手往身側收、確認護腕遮住烙印。grok 那份最深——寫烙印「忽然發熱」、他刻意不去碰。

同一批真實輸出順便驗了包 A 的解析器與各家用量形狀：

- agy 那 204 個增量字元串起來與 `result.response` 一字不差，用量三欄全對、契約判別走第一式（`total == input + output`，即 input 已含 cache_read）。
- codex 冷啟動的 `cached_input_tokens` 正好是 9,984——只命中它自己的固定前綴，與本 plan 的分析吻合。
- grok 冷啟動 `input 3,920 ＋ cache_read 10,368 ＋ output 1,224 = total 15,512`：**input 不含 cache_read**（與 codex／agy 相反，現有 parser 的註解正確），10,368 是它自己的固定前綴。花費 $0.0204，四支 CLI 只有它與 claude 報金額。

包 A 驗收四項：正文逐字不變、沒有重複 final text、錯誤事件仍會失敗、usage 正確落帳。

包 B 完成後做**成對測試**（同角色／換角色 × 冷／暖），記**絕對 cached tokens** 而不只看百分比；**codex 要先扣掉固定的 9,984**，否則 90% 可能只是它自己的前綴在中。

快取實測基線：codex 15 筆（底線 9,984）、api `stealth/ox-alpha` 4 筆（底線 64）、grok 4 筆（89.1／48.6／49.0／91.8%，同樣的雙峰）。三條路各自獨立證實「換角色全滅、同角色連續全中」。agy 要先改 `stream-json` 才量得到。`deepseek-v4-pro-0813-free` 27 筆命中率恆 0（有回報、值就是 0），拿它測共線量不到任何東西。

## 保溫的既有結論（api-cache-visibility 查證所得，供結論 4 參考）

- OpenRouter **不支援** `max_tokens: 0` 零輸出預熱：實測 HTTP 200 但照常生成完整回覆，等於把 0 當成沒有上限。
- `max_tokens: 1` 有效（實測設 16 就是 16，`finish_reason: "length"`），所以保溫＝重送 messages ＋ `max_tokens: 1` ＋ `stream: false`，成本是「讀價 × 輸入 ＋ 輸出價 × 1」。
- API 無狀態，保溫呼叫不留痕跡，不需要 claude lane 那套 `truncate_ping`。
- 這條線的快取 TTL 落在 3 分鐘（活）與 21 分鐘（死）之間，尚未收斂。

## 工作量估計

- **包 A｜agy stream-json（約 80 行）**：`agy_args` 加旗標 5、`parse_agy_line` 改吃 NDJSON 事件 30、`parse_agy_usage` 15、測試 30。
- **包 B｜共線組裝器（約 125 行）**：新組裝函式 40、`lib.rs` 改呼叫 15、`flatten_messages` 修前綴 10、測試 60。
- **包 C｜anthropic block（約 70 行）**：組裝器交付穩定邊界＋`anthropic_messages` 改用該邊界 30、測試 40。延後。
