# stable-free-failover：穩定免費自動避開不能用的模型

首發前必做〔作者裁決 2026-10-04〕。起因：2026-10-04 代測，穩定前兩名都是 Google AI Studio 共用池的 gemma，24 次派送 14 次 429，預設設定的新玩家一句都送不出去。

三件事（立案原意，見 [handoff](../handoffs/archive/stable-free-failover.md)）：開 App／背景刷新試打選目前模型；對話中連續 2 次模型層級失敗換下一支並在聊天室明講；穩定推薦擴到 4 支、同一上游不連著上榜。

## 作者裁決（2026-10-04）

- 換模後自動重送這一句，只重送一次〔作者裁決 2026-10-04〕。
- 已經吐出正文就不自動重送：換模照樣提交並明講，下一句用新模型〔作者裁決 2026-10-04〕。
- 推薦名單排不開上游時就少列並標示降級，不硬湊滿 4 支〔作者裁決 2026-10-04〕。
- 沒有任何證據的 429 歸 `UnknownRateLimit`，走有限 failover（一樣連續 2 次才換、最多重送一次），保留原錯誤，不宣稱是上游限流〔作者裁決 2026-10-04〕。
- exhausted 冷卻 30 分鐘，起算點見 §4.4；冷卻清空時失敗計數同步歸零；冷卻只代表可以重試，不增加試打頻率〔作者裁決 2026-10-04〕。
- 穩定候選與限時推薦只收輸出只有文字的模型（輸入維持含文字即可），排除音樂模型這類會同時輸出音訊的模型〔作者裁決 2026-10-04〕；規則版本記在快取，舊快取下次刷新就重抓，不等 TTL。
- 4 支名單只要求相鄰不同上游，不限制每家只能 1 支〔作者裁決 2026-10-04〕：免費來源少，限制成每家 1 支會讓名單太短（2026-10-04 實測只有 Google AI Studio、Nvidia 兩家輪流）。

## 與 free-player-onboarding 2026-09-18 裁決的衝突（新取代舊）

[free-player-onboarding](free-player-onboarding.md) 以下條文由本案取代，結案時回寫該檔：

| # | 舊（2026-09-18） | 新〔作者裁決 2026-10-04〕 |
|---|---|---|
| 1 | §4／§7：失敗只回報、不換模、不因失敗改綁 | 單次失敗仍只回報；連續 2 次模型層級失敗換下一支 |
| 2 | §4「為何」：優先維持文風一致，不靜默換模 | 作者接受換模；換時在聊天室明講 |
| 3 | §7 deferred：連續 N 次失敗跳窗問玩家 | 不問，自動換＋聊天室提示 |
| 4 | §7：失敗就回報這次請求失敗，玩家再送 | 換模後同一句自動重送一次（未吐正文時） |
| 5 | §8：目前模型＝RP 第一名，只在背景刷新排行換人才變 | 目前模型＝本機記錄（試打或換模結果），見 §4 |
| 6 | §6.2／§10／§13／§14／§18：穩定區最多 2 支 | 最多 4 支、上游分散、排不開就少列；第 2–4 名同時是換模順序，也仍可手動點選固定 |
| 7 | §5「資料都不是模型呼叫、不佔每日次數」 | 仍成立；另加試打（佔次數，作者接受）與第四份資料 endpoints（不佔） |
| 8 | §10／§16「目前使用（預測）」＝第一名 | 顯示本機記錄的目前模型 |
| 9 | §12／§18 驗收「失敗不自動切到下一支」 | 改為「單次不切、連續 2 次切」 |

不變：每次派送只帶一支模型、不送備援陣列；限時／stealth 不進自動路徑；`recommended`／手動模式不換模、不試打（§15）；CLI 路線不動。

## 1. 官方 API 查證（2026-10-04）

| 問題 | 結論 | 出處 |
|---|---|---|
| 模型的上游 | `GET /api/v1/models/{author}/{slug}/endpoints` 的 `data.endpoints[].provider_name`／`tag`。路徑帶 `:free` 只回免費端點（實查 `google/gemma-4-31b-it:free` → Google AI Studio；`nvidia/nemotron-3-super-120b-a12b:free` → Nvidia）。uptime 欄位對免費端點常為 null，不採用 | API reference「List endpoints」＋實打官方 API |
| `/models`、`/models/user` 有沒有上游 | 沒有；`top_provider` 無名字 | 實查 |
| 最小試打 | 非串流 `POST /chat/completions`，`messages=[{role:user,content:"hi"}]`、`max_tokens: 1`（文件：1 or above）。小 `max_tokens` 可能被 reasoning 吃光回空內容＋`length`，所以 2xx 且無 `error` 即算可用。文件沒寫會不會扣每日次數，一律當作會扣 | 「Parameters」 |
| 帳號層級 | 402＝`payment_required`；每日：`GET /api/v1/key` 的 `free_model_daily_requests.{used,limit,remaining}`；另有 20 RPM，不在 `/key`。超限回 429 並帶 `X-RateLimit-Limit/Remaining/Reset` 說明是哪個限制。429／503／部分 402 帶 `Retry-After` | 「Limits」「Errors and debugging」 |
| 上游限流 vs 平台限流 | 文件沒有直接分辨法：都是 429＋`error_type: rate_limit_exceeded`。`metadata` 可能帶 `provider_name`／`provider_code`／`raw`，但沒說只出現在上游錯誤，`raw` 也可能只是錯誤原文。串流中途錯誤是 200 後的 SSE `{error:{code,message,metadata}, choices:[{finish_reason:"error"}]}`，沒有失敗當下的 HTTP headers | 同上 |

**待實證**：上游 429 是否也帶 `X-RateLimit-*`。若是，§2.2 會把上游 429 全判成平台限流、永遠不換模——實機驗收必須抓真實上游 429 與平台 429 各一筆回填本表，判據照實調整。

## 2. 凍結契約（施工前定案，A／B 兩包共讀）

### 2.1 結構化錯誤 `ApiFailure`（transport/client.rs）

`stream_chat_models` 失敗改回結構，不再只回字串：

```text
ApiFailure {
  stage: Http | Stream | Network | Timeout | Cancelled,
  status: Option<u16>,                 // HTTP 狀態；SSE 錯誤取 error.code
  rate_limit: Option<{limit, remaining, reset}>,  // X-RateLimit-* 原值；SSE 失敗為 None
  retry_after_secs: Option<u64>,       // Retry-After：delta-seconds 或 HTTP-date 皆解析，換算成距今秒數
  code: Option<i64>, error_type: Option<String>,
  provider_name: Option<String>, has_raw: bool,   // 解析自完整 body／SSE error，不截斷；只當線索
  emitted_text: bool,                  // 失敗前是否已 on_delta 過任何正文
  display: String,                     // 給前端的字串，沿用現行 http_error／AI_* 格式
}
```

- body 在截 2000 字之前完整解析；`display` 照舊截斷。其他路徑（`stream_chat`、Responses）不改。
- `Retry-After` 只記錄、進 harness 留痕，不影響重送：重送一律換到另一支模型，原模型的等待時間與它無關；平台限流本來就不重送。也不延後試打（試打節流照 §5）。

### 2.2 失敗分類（純函式）

輸入 `ApiFailure`＋可選的 `/key` 查詢結果，輸出六類之一：

| 類別 | 條件 | 計數 | 
|---|---|---|
| `Cancelled` | 取消信號 | 不計，忽略 |
| `Account` | 401／402／403；429 且有平台證據：`X-RateLimit-Remaining` 為 0、訊息含 `free-models-per-day`／`free-models-per-min`、或 `/key` 查到每日剩餘 ≤ 0 | 不計，忽略 |
| `Model` | 502／503／408／504、`Network`、`Timeout`；SSE `error_type` 為 `provider_overloaded`／`provider_unavailable`／`timeout` | 計 1 |
| `Gone` | 404（模型下架） | 直接當已達 2 次 |
| `UnknownRateLimit` | 429（HTTP 或 SSE `code`）且沒有平台證據——含 `/key` 查詢失敗、查到剩餘 > 0、SSE 沒有 headers 等情況。`provider_name`／`raw` 只記錄當線索，不當上游確證 | 計 1（有限 failover，與 `Model` 同規則）〔作者裁決 2026-10-04〕 |
| `Other` | 內容過濾、拒答、空回應、不完整、其他 400 | 不計，忽略 |

- 無平台 headers 的 429 才查 `/key`（可被取消）；查詢結果只用來排除「每日用完」，`remaining > 0` 不排除 RPM，所以仍歸 `UnknownRateLimit` 而不是上游。
- 回給前端：`Model`／`Gone` 掛 `AI_FREE_MODEL_BUSY:` 前綴（`ai_call_failure` 白名單加這個碼）；`UnknownRateLimit` 保留原錯誤 `display`，不宣稱上游擁擠；其他照現行 `display`。

### 2.3 重送限制

- 一次邏輯呼叫（一個 command 內的一次 `stream_*_via_transport`）最多派送 2 次。
- 第 2 次派送只在：第 1 次失敗使換模**成功提交並落盤**（§4.3），且第 1 次 `emitted_text == false`。
- 已經吐出正文就不自動重送〔作者裁決 2026-10-04〕：前端累加 delta、後端取消時也累加 buffer，直接重送會把兩支模型的半截正文接在一起，按停止時甚至把混合文字落盤。這時照常回報失敗，換模照樣提交、明講，下一句用新模型。
- 第 2 次派送也照 §4.2 取票核對：只有「非替代、可計數」（`Model`／`Gone`／`UnknownRateLimit`）的失敗才對它的 `selected_model` 記 1 次；替代、`Account`、`Other`、`Cancelled` 都不計。無論結果如何，本呼叫內不再換模、不再重送（第 2 發遇到 `Gone` 也只把計數記到門檻，下一次可計數失敗才換）。第 2 次是替代模型而成功時，同樣不歸零全域計數。
- 第 2 次的候選照 §4.5「重送選模」：排除第 1 次的 `selected_model` 與 exhausted；沒有合格候選就不派第 2 次，保留已提交的換模，回報第 1 次的失敗。
- 記錄第 1 次失敗、換模、取重送票在同一次鎖內完成；取重送票時同樣重讀磁碟上的權威設定與名單，送出設定已過期、帳號或名單世代已變就不重送。
- 重送留在同一 command／turn、沿用同一份 messages，不重新 append 玩家句；只有最終失敗才讓前端走既有 `discardPlayer`（收據、尾行核對、世代條件不變）。
- 取消信號要能打斷 `/key` 查詢與重送；取消一律 `Cancelled`。
- 兩次派送各自記一行用量帳本（各帶自己的 model）。

### 2.4 事件 payload（後端由包 A 實作）

```text
smart-free-failover      { eventId, world: string|null, turnId: string|null, from, to, retried: boolean }
smart-free-model-switched { eventId, world: string|null, turnId: string|null, model }
```

- `eventId`：每次發事件新產生的 ULID，不重用。`from`／`to`／`model` 是顯示名。
- `turnId` 一路接完整：聊天輪（`commands/chat.rs` 已用 `inflight::register_turn(world, turn_id)`）把 turn id 傳進 `stream_turn_via_transport`／`stream_turn_reporting_truncation` 再到 smart_free 分支；非聊天輪呼叫（換幕摘要、翻譯、重構、開桌、生圖）給 null。

### 2.5 提示去重與防舊事件

- 後端：同一邏輯呼叫已發過 `smart-free-failover`，其重送成功時 `record_responder` 的換手不再發 `smart-free-model-switched`。一般換手（不在 failover 呼叫內）照發，每次各有新 `eventId`，不會被吞。
- 前端：
  - 以 `eventId` 去重（同一事件只顯示一次）。
  - 聊天室只接受 `turnId` 屬於「本次檢視期間由這個聊天畫面發出的 turn」的事件：聊天控制器記住自己送出的 turn id 集合，換幕、離開該桌、畫面卸載就清空；同桌換幕後或離開再回來收到的舊事件因 turn id 不在集合內而丟棄。
  - `turnId` 為 null 的事件（非聊天輪）不進聊天室，改用 App 層的非阻塞輕量提示（與 `SmartFreeNewModelBanner` 同級），不跳 modal。
- 兩種提示在聊天室都是一行非持久系統提示，不寫進逐字稿（否則會進提示詞）；舊的 switched modal 拿掉。

### 2.6 測試通道注入

現行智慧免費只在官方網址下啟用，換 `base_url` 會讓 `is_active` 退出，假端點測不到。新增 `test-harness` feature 限定的 OpenRouter 來源覆寫：harness 指令設定後，`smart_free` 的 api／probe 與 `stream_chat_models` 改打覆寫網址，`is_active`／`tracks_openrouter` 把覆寫網址視同官方。release build 沒有這個入口。假端點要能逐請求腳本化回應（狀態、headers、body、SSE 中途錯誤、延遲）。

## 3. 排名：最多 4 支、上游分散

- `STABLE_RECOMMENDATION_COUNT` 改 4；現行 `stable_fallback_models`（送出用）與 `recommendation_models` 的穩定區（設定頁）合併成一個函式產出「名單」（lineup），兩邊共用。
- 原名次來源不變：RP 排行 → 七日榜 → 其他合格穩定（`is_stable` 不變）。名單以 `RESERVED_OUTPUT_TOKENS` 的最小長度篩選；單句長度見 §4.5。
- 上游：每支穩定候選的免費端點 `provider_name` 集合，抓**全部**穩定候選（數量本來就少，且不佔次數），存快取 `upstreams: {model_id: [provider_name…]}`＋`upstreams_fetched_at`，TTL 12 小時；單支抓失敗保留舊值，沒有舊值記為「未知」。兩支已知集合有交集＝同上游。
- 只要求相鄰不同上游、不限制每家支數（實測常只有兩家輪流）〔作者裁決 2026-10-04〕。
- 重排規則（貪婪）：第 1 名固定是原名次第 1；之後每個位置取「剩下的候選中原名次最高、且與前一支不同上游」的那支。例：A1,A2,B1,B2 → A1,B1,A2,B2。找不到就停，少列〔作者裁決 2026-10-04〕。
- 上游未知的模型：比較相鄰時視為「與任何模型都不同上游」（可以入列、也不擋別人），但只要名單含未知上游，或少於 4 支（不論是排不開還是候選本來就不夠），名單就標 `diversified: false`。設定頁在 false 時顯示降級說明；文件與介面都不宣稱保證分散。
- 名單世代 `lineup_gen`＝名單 id 依序的雜湊。

## 4. 目前模型與狀態

### 4.1 狀態、世代、落盤

本機新檔 `smart_free_current.json`：

```text
{ account, lineup_gen, revision, model,
  confirmed_at: Option,              // 試打 confirmed 的時間；聊天成功不寫（免每句落盤），只在要清 exhausted 時落盤〔主線核准、Sol 驗收接受〕
  probe: { last_run_at, outcome },   // outcome 見 §5
  exhausted: [model…], exhausted_at: Option }
```

程序內持有同一份狀態（以 root 為鍵）＋聊天失敗計數 `{model, count}`＋**選模世代 `epoch`**，全部由一把 std Mutex 保護；鎖內不 await，計數、選模、提交都在短鎖內完成。

- `epoch`：程序內單調遞增、不重用的整數（票不跨程序，所以不必落盤）。以下任何一件事都 +1：目前模型改變（換模、試打選到別支、重建）、`api_model_mode` 改變、OpenRouter 金鑰指紋改變、`base_url` 改變、`lineup_gen` 改變。寫入這些設定的地方（設定頁存檔、OAuth 完成、手貼金鑰）之後都會 `warm`，由它比對設定鍵推進；取票時也會用磁碟上的最新設定再比對一次。快取刷新換到新名單時，當下就重建並推進（不等下一次取票）；刷新成空名單也推進，作廢在途的舊票。失敗計數綁 epoch，epoch 一推進就歸零。A→B→A、stable→recommended→stable 這類來回會讓 epoch 前進兩次，舊票必然對不上。
- 提交流程：鎖內複製狀態成待寫版本，把 `revision + 1` 與所有改動寫進待寫版本 → 原子寫檔 → 寫成功才把記憶體整份換成待寫版本，再發事件。磁碟與記憶體的 revision 一律相同。寫檔失敗（含更新閘門關閉）：記憶體不變、不發提示、不重送，照常回報原失敗；計數保留在記憶體。
- 帳號指紋或 `lineup_gen` 與檔案不符 → 視為無記錄，重建（model＝名單第 1，清 exhausted 與計數，epoch +1）。重建落盤失敗：記憶體不變但 epoch 照樣推進（先前的票全部作廢），這一句只發不可計數的票。

### 4.2 票與晚到結果

派送與試打開始前在鎖內取票。聊天取票時不用呼叫開始前備好的素材，而是在鎖內重讀磁碟上的設定與快取（權威名單、帳號、設定鍵）；送出用的設定與磁碟最新設定不同、或重建落盤失敗時，照樣送出但票標成不可計數（結果一律不計、不換模、不重送）：

```text
Ticket { account, epoch, revision, lineup_gen, current_model, selected_model, substitute: bool, countable: bool }
```

`selected_model` 是實際送出的那支；`substitute` 表示這是 §4.5 的本句替代；`countable = false` 的票結果一律不計。試打用另一種票 `{account, epoch, revision}`。await 回來後在鎖內核對：

- 聊天結果：`account`、`epoch` 相同才處理；`substitute == true` 的結果一律不計（成功不歸零目前模型的計數、失敗不加），只記 pins 與用量；否則照 §4.3 計入目前模型。
- 試打結果：見 §5。
- 並行結果依「回到鎖內的先後」處理。

### 4.3 連續與換模

- 「連續」＝目前模型（同一 epoch）上，依提交順序排列、被計入的結果。只有**成功**會歸零；`Other`／`Account`／`Cancelled` 及替代模型的結果都直接忽略——既不歸零也不加，所以 `Model → Other → Model` 與 `Model → Account → Model` 都算連續 2 次。
- 計數達 2（`Gone` 直接達 2）時：
  1. 待寫版本把目前模型加進 `exhausted`，`exhausted_at = now`。
  2. 目標＝名單中排在它後面、不在 `exhausted` 的第一支；到底就從名單頭找。目標只看名單與 exhausted，**不看本句長度**（全域目前模型反映擁擠，不反映這一句多長）。
  3. 有目標：目前模型改成目標、計數歸零、epoch +1，照 §4.1 提交；成功後發 `smart-free-failover`，符合 §2.3 才重送（重送時照 §4.5 選本句模型，可能是替代）。
  4. 沒目標（名單全在 exhausted）：仍提交 exhausted 與 `exhausted_at`（落盤），目前模型不動、計數歸零，本次回報 `SmartFreeAllBusy`，不重送。

### 4.4 exhausted 冷卻〔作者裁決 2026-10-04〕

- 起算點：`exhausted_at`＝最近一次有模型被加進 exhausted 的時間（§4.3 第 1 步；沒目標時也更新）。
- 滿 30 分鐘後，下一次取票時在鎖內清空 exhausted、`exhausted_at = None`、失敗計數同步歸零，照 §4.1 提交。目前模型不動。
- 冷卻只代表允許再試這些模型，不代表恢復；不觸發試打、不改試打節流。
- 其他清空時機：任一非替代聊天成功、試打 `confirmed`、`lineup_gen` 變、換帳號——同樣連計數一起歸零。

### 4.5 單句長度（context eligibility）

- 選本句模型：目前模型放得下 `required_context(messages)` 就用它（`substitute = false`）。
- 放不下：依序找名單內其他支，再找名單外的合格穩定候選（原名次順序，限時／stealth 不算）——與現行 `pick_primary` 搜全部穩定候選的範圍相同，不縮小。找到即為本句替代（`substitute = true`），只用於這一句、不改全域目前模型、不計失敗、不觸發換模。
- 全部穩定候選都放不下：照現行回 `NoFreeModels`（長度問題），不回「都擁擠」。
- **重送選模**（§2.3 第 2 次派送）：同上順序，但候選一律排除第 1 次的 `selected_model`，也排除 `exhausted` 內的模型（剛被判擁擠，重送不該再送回去）。例：A 放得下本句、連敗 2 次、全域換到 context 較短的 B；B 放不下本句時，替代搜尋跳過 A，找名單內外其他放得下的穩定模型；都沒有就不派第 2 次，換模到 B 照樣保留，回報第 1 次的失敗。

### 4.6 其他讀取點

`status()`、設定頁「目前使用」、`models_in_use`、harness `preview_primary` 都改讀目前模型。pins 仍只記實際成功回話者（顯示＋到期提醒）。

## 5. 試打

一輪＝從名單第 1 名往下逐支送最小請求，第一支成功就停（通常 1 發）。

- 只在 `stable_free` 且有金鑰時跑；同時只跑一輪（tokio Mutex）。拿到鎖之後重查一次節流，不符就直接結束，避免多個觸發串行重複扣次數。
- 開跑前查 `/key`：剩餘 ≤ 5 不打；查不到照打。
- **每打一發之前**，在鎖內核對帳號、epoch，並重讀 config 確認仍是 `stable_free`；不符就停（`stopped(stale)`），不再扣下一發。
- 停損：任一發判成 `Account` → 整輪停止。
- outcome 分類（每輪必記其一）：
  - `confirmed(model)`：有一發成功。
  - `all_failed`：每一發都是 `Model`／`Gone`／`UnknownRateLimit`。
  - `inconclusive`：沒有成功，且至少一發是 `Other`。
  - `stopped(account | low_quota | stale | cancelled)`。
- 提交：
  - `probe.last_run_at`、`probe.outcome`：只要打過任何一發就提交（次數已花掉，節流要算進去），不論 outcome；但只在票的 `account` 與鎖內最新狀態相同時，從最新狀態合併這兩個欄位（不覆蓋其他欄位）。帳號已換就整筆丟棄，不寫進新帳號的紀錄（舊帳號的節流不保留，換 key 本來就重建狀態）。
  - 選模欄位（目前模型、`confirmed_at`、清 exhausted）只在 `confirmed` 且票的 `account`、`epoch`、`revision` 都未變時提交；`revision` 變了表示期間聊天已換模或冷卻清空，整輪的選模結果丟棄。`confirmed` 選到別支時 epoch +1。
  - `all_failed`／`inconclusive`：目前模型不動（無記錄才設名單第 1），不寫 `confirmed_at`。
- 試打結果不進聊天失敗計數、不進用量帳本；走 harness `ai_dispatch("api-smart-free-probe", …)` 留痕。單發逾時 20 秒。
- 觸發與節流（冷卻清空不屬於觸發條件）：
  - App 啟動：`last_run_at` 距今 ≥ 1 小時，或無記錄，或 `lineup_gen` 變了。
  - 背景迴圈（每小時檢查）：`lineup_gen` 變了 → 跑一輪；目前模型不是第 1 名且 `last_run_at` 距今 ≥ 3 小時 → 只試第 1 名到目前模型前一支。
  - OAuth 完成（現行 `warm`）：refresh 後跑一輪。
  - 不在送出前試打，不擋開 App、不擋聊天。

## 6. 對話中流程（dispatch.rs，stable_free 分支）

```text
取票（§4.2，含冷卻檢查 §4.4）→ 選本句模型（§4.5）→ 派送 1
  成功 → 計入（替代則不計）、record_responder（§2.5 去重）→ 回傳
  失敗 → 分類（必要時查 /key，可被取消）→ 計入（替代則不計）
        未達換模 → 回報
        達換模、有目標、提交成功 → 發 failover 事件
             emitted_text 為 false → 重送選模（§4.5，排除第 1 發與 exhausted）
                  有候選 → 取票 → 派送 2 → 成功回傳（替代不歸零）／失敗依 §2.3 計數並回報，不再換模
                  無候選 → 不派第 2 發，回報第 1 次的失敗（換模保留）
             emitted_text 為 true  → 回報原失敗（不重送）
        達換模、沒目標 → 提交 exhausted → 回報 SmartFreeAllBusy
        提交失敗 → 回報原失敗
```

## 7. 設定頁

- 穩定區列名單（最多 4 支，可能少列）：第 1 名 radio＝`stable_free` 自動模式；第 2–4 名點選＝`recommended` 固定該支（現行）。
- 「目前使用」標在目前模型那一列，拿掉「（預測）」；`diversified: false` 時顯示一句降級說明。
- 第 2–4 名理由下方加「第一名擁擠時自動改用」。

## 8. 要改的檔

Rust：
- `transport/client.rs`：`ApiFailure`（含 `Retry-After` 兩種格式解析）、`StreamOutcome` 保留 code／error_type／metadata、`emitted_text`；`stream_chat_models` 回結構化錯誤；OpenRouter 來源覆寫讀取點（test-harness）。
- `smart_free/api.rs`：`fetch_upstreams`、`probe`。
- `smart_free/select.rs`：名單合併、4 支、分散重排、`diversified`。
- `smart_free/store.rs`：`Cache.upstreams`、`smart_free_current.json`。
- 新 `smart_free/failover.rs`：狀態、epoch、取票／核對、分類、換模、冷卻、試打流程（`mod.rs` 已 827 行）。
- `smart_free/mod.rs`：`prepare_call`／`status`／`models_in_use`／`preview_primary` 改讀目前模型；`warm`／`spawn_background_refresh` 接試打；`record_responder` 去重；`bump_epoch`。
- `transport/dispatch.rs`：§6 流程、取消接線、`turn_id` 參數、事件 payload（含 `eventId`）、`ai_call_failure` 白名單。
- `commands/chat.rs`：把既有 turn id 傳進 dispatch。
- 寫入模式／金鑰／base_url 的地方（`commands/settings.rs`、`openrouter_oauth.rs`、`data/config.rs` migration）：呼叫 `bump_epoch`。
- `harness/`：OpenRouter 來源覆寫指令、假端點腳本化回應。

前端：
- `src/App.tsx`：拿掉 switched modal listener；`turnId` 為 null 的兩種事件改走 App 層非阻塞提示。
- `src/features/play/`：聽兩種事件、`eventId` 去重、本檢視期間的 turn id 集合、聊天室非持久系統行。
- `src/features/settings/SettingsForm.tsx`：最多 4 支、目前使用、降級說明、備援說明。
- `src/shared/ui/ai-error.ts`：`AI_FREE_MODEL_BUSY` → `errFreeModelBusy`。
- `src/i18n/features/smart-free.ts` 與錯誤文案檔：新鍵十語系。

## 9. 十語系新文案（zh-TW 定稿，其他九語照意翻）

| 鍵 | zh-TW |
|---|---|
| `smartFreeFailover` | {from} 目前擁擠，已改用 {to}。 |
| `be_smart_free_all_busy`（後端 `UiMsg::SmartFreeAllBusy`，包 A 補十語系；check-i18n 要求每個 UiMsg 都有 `be_*` 文案，包 B 不另加前端鍵）〔主線核准、Sol 驗收接受〕 | 穩定免費的模型目前都擁擠，請稍後再試。 |
| `smartFreeBackupHint` | 第一名擁擠時自動改用 |
| `smartFreeCurrentLabel` | 目前使用 |
| `smartFreeNotDiversified` | 目前可用的免費模型集中在少數供應商，擁擠時可能一起不能用。 |
| `errFreeModelBusy` | 這支免費模型目前擁擠，稍後再送一次就好。 |

`smartFreeFailover` 用於 `Model`／`Gone`／`UnknownRateLimit` 觸發的換模都一樣；「擁擠」是給玩家的白話，不宣稱是上游限流，原錯誤照樣附在失敗訊息小字。`smartFreeSwitched` 沿用文字、改在聊天室顯示。人眼審校併進 i18n-more-languages。

## 10. 測試

Rust 單元（failover 核心寫成吃事件的純狀態機）：
- 分類表：429＋`X-RateLimit-Remaining: 0`、429＋`free-models-per-day`、429＋`provider_name`（仍歸 `UnknownRateLimit`）、429 無證據＋`/key` >0／≤0／查詢失敗、SSE 中途 429（無 headers）、SSE `provider_overloaded`、402／401／403、502／503／504、404、逾時、取消、內容過濾、空回應；超過 2000 字的 body 仍解析得到 metadata；`Retry-After` 秒數與 HTTP-date。
- 排名：A1,A2,B1,B2 → A1,B1,A2,B2；A1,B1,B2,B3 → 2 支並 `diversified: false`；未知上游可相鄰任何模型並標 false；RP→weekly→available 名次主導；限時／stealth 不入列。
- 世代與落盤：ABA（目前 A → 換 B → 換回 A，舊 A 票晚回被 epoch 擋下）；stable→recommended→stable 晚回被擋；提交後磁碟與記憶體 revision 相同；寫檔失敗時記憶體、事件、重送都不動。
- 連續：`Model→Other→Model`、`Model→Account→Model` 換模；成功歸零；替代模型成功不歸零、失敗不加。
- exhausted：沒目標時 exhausted 與 `exhausted_at` 落盤、計數歸零、回 AllBusy；冷卻滿 30 分鐘清空並歸零計數、不觸發試打；其他清空時機。
- 長度：目前模型放不下時用名單內替代、再用名單外長 context 穩定模型；全部放不下回 `NoFreeModels` 而非 AllBusy。
- 重送選模：A 連敗 2 次換到 B、B 放不下本句 → 重送不挑回 A、也不挑 exhausted；有其他放得下的就送它（替代），沒有就不派第 2 發、換模保留、回報第 1 次失敗。
- 第 2 發計數：非替代的 `Model` 失敗記 1；替代失敗、`Account`、`Other`、`Cancelled` 不計；任何結果都不再換模；替代成功不歸零全域計數。
- 試打：換 key → 新帳號試打完成並記 `last_run_at` → 舊帳號試打晚回，新帳號的 `last_run_at`／`outcome`／選模欄位都不變；同帳號晚回只合併 probe 兩欄。第一支成功即停；`Account` 停整輪；每發前核對（中途切 `recommended` 就不打下一發）；混到 `Other` 記 `inconclusive`；`revision` 變了選模結果丟棄但 `last_run_at` 照記；拿鎖後重查節流（兩個觸發只跑一輪）；`all_failed` 不寫 `confirmed_at`、背景不每小時重打。
- 事件：同一 revision 下多次合法 switched 提示各有 `eventId`、都會發；failover 後重送成功不再發 switched；payload 帶 world／turnId。

Rust async 膠水（tokio 測試，注入可阻塞的假傳輸，不只測狀態機）：取消在 `/key` 查詢中、在重送派送中都能立即返回 `Cancelled`、不計數、不發第二發；重送最多 2 發；兩次派送各記一行用量；`emitted_text` 時不重送。

前端 vitest：提示不寫進逐字稿；`eventId` 去重；換幕後、離開再回來收到的舊 turn 事件不顯示；別桌事件不顯示；`turnId` null 走 App 層提示；重送成功不呼叫 `discardPlayer`、最終失敗才呼叫，半截失敗與取消不改原收據／尾行契約；`AI_FREE_MODEL_BUSY` 文案；設定頁少列、目前使用、降級說明。

測試通道（§2.6 注入，零真實請求）：啟動試打第 1 支 429、第 2 支成功 → 目前模型＝第 2 支；聊天連 2 次 503 → 聊天室一行換模提示、同句重送成功、逐字稿只有一個玩家句；402 不換模；平台 429（帶 `X-RateLimit`）不換模；無證據 429 連 2 次 → 換模、原錯誤保留；半截正文後失敗不重送。

實機（真模型，排實測佇列，不能以假端點通過代替）：預設設定開 App 送一句，擁擠時自動換到別家上游並看到提示；抓真實上游 429 與平台 429 各一筆回填 §1、確認 §2.2 判據。

## 11. 分包

§2 契約凍結後才開工。

- **包 A（Rust＋測試通道注入，派 Opus：併發、落盤、取消）**：§2.1–§2.3、§2.4 後端 payload 與 `turnId` 接線（含 `commands/chat.rs`）、§2.5 後端去重、§2.6、§3–§6、`bump_epoch` 接點與對應 Rust 測試。可獨立 verify；前端未接時事件無人聽、行為仍正確。
- **包 B（前端＋十語系，可派 Sonnet）**：§2.5 前端半、§7、§9 與前端測試。
- 「聊天室明講」要等 B 接好才驗收；最後另驗真模型與真 429。
