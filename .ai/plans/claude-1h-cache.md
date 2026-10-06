# claude-1h-cache 規格

## 一、裁決

- 停 claude 保溫 ping、過期門檻改 1 小時、省額改 2 倍寫入係數〔作者裁決 2026-10-06〕。
- 1h 不是 CLI 固定行為，選 A：app 呼叫 claude CLI 時帶 `CLAUDE_CODE_PROMPT_CACHE_TTL=1h` 釘死；玩家用到超額時只跳提示，不替玩家做花錢或改設定的決定；帳本記每輪實際快取時效〔作者裁決 2026-10-07〕。
- 離開提醒撤掉；只釘續聊線，單發呼叫不帶 1h〔作者裁決 2026-10-07〕。

## 二、證據（claude 2.1.287）

CLI 判定順序（執行檔 `ejt`→`L$t`）：
1. `FORCE_PROMPT_CACHING_5M` → 5m。
2. `CLAUDE_CODE_PROMPT_CACHE_TTL`（主對話來源；`-p` 屬主對話，實跑生效）→ 照設。
3. settings `promptCacheTtl` → 照設。
4. `ENABLE_PROMPT_CACHING_1H` → 1h。
5. 自動：訂閱登入、`isUsingOverage` 不為 true、來源在遠端 allowlist → 1h；否則 5m。

超額旗標只在第 5 步參與，**帶了環境變數就算超額也照樣 1h**（以超額計價）。環境變數不分登入方式，相容端點（`claude_base_url`）也會送 `ttl:1h`。

實跑（Sonnet、app 旗標、每發 system 加亂數；表內為 result `usage.cache_creation`）：

| 條件 | 1h | 5m | cost_usd |
|---|---|---|---|
| 預設（無覆寫） | 8,207 | 0 | 0.0334 |
| env `TTL=1h` | 8,682 | 0 | 0.0348 |
| env `TTL=1h`＋`FORCE_PROMPT_CACHING_5M=1` | 0 | 8,682 | 0.0217 |
| env `TTL=1h`＋`--settings {"promptCacheTtl":"5m"}` | 8,680 | 0 | — |
| env `TTL=1h`＋`--settings {"env":{"CLAUDE_CODE_PROMPT_CACHE_TTL":"5m"}}` | 8,678 | 0 | — |
| env `TTL=1h`＋`--settings {"env":{"FORCE_PROMPT_CACHING_5M":"1"}}` | 0 | 8,682 | 0.0217 |
| env `TTL=1h`＋`FORCE_PROMPT_CACHING_5M=`（空字串） | 8,679 | 0 | 0.0348 |
| env `TTL=1h`＋`FORCE_PROMPT_CACHING_5M=0` | 8,678 | 0 | 0.0348 |

結論：
- 唯一蓋得過我們的是 `FORCE_PROMPT_CACHING_5M`，行程環境或設定檔 `env` 區塊都算。行程環境那份由續聊線自己的 envs 帶 `=0` 壓掉（實跑生效，不必改共用 runner）；玩家 `~/.claude/settings.json` 的 `env` 那份擋不住（`--safe-mode` 下是否仍套用未驗，不改玩家設定就測不了），靠帳本記實得時效兜底。
- 實際時效可靠來源：result 事件 `usage.cache_creation.ephemeral_1h_input_tokens`／`ephemeral_5m_input_tokens`。
- 超額可靠來源：`-p` 每發都吐 `{"type":"rate_limit_event","rate_limit_info":{…}}`（實跑八發皆有；樣本順序為 system init → assistant → rate_limit_event → post_turn_summary → result，但不保證在 result 前），欄位 `isUsingOverage`（bool）、`overageStatus`（allowed／allowed_warning／rejected）、`overageDisabledReason`。API key／Bedrock／Vertex 不帶。本帳號 `overageDisabledReason=org_level_disabled`，`isUsingOverage:true` 的樣本無法實跑。

## 三、施工

### 1. 只在續聊線釘 1h
- 新增 `pin_lane_cache_ttl(&mut LaneCall)`：provider 是 Claude 時追加 `CLAUDE_CODE_PROMPT_CACHE_TTL=1h` 與 `FORCE_PROMPT_CACHING_5M=0`。
  - 只在 `commands/chat.rs` 的角色線和 `gm_lane_reply` 拿到 `prepare_lane_call` 結果之後呼叫。
  - `prepare_lane_call` 本身、卡重構 `refactor_ai/session.rs`（短命 session，不屬續聊線）、共用的 `claude_cli_envs`、`cli/runner.rs`、單發 `stream_via_transport` 都維持原環境。
- 玩家設定檔 `env` 裡的 FORCE 不處理，由第 2 點的實得時效呈現〔模型判斷·未裁決〕。

### 2. 用量回填 lane，並估計 lane 時效
**解析**：`PromptCacheUsage` 加 `created_1h_tokens: Option<u64>`，只有 `parse_claude_usage` 填（讀 `usage.cache_creation.ephemeral_1h_input_tokens`）。若該欄位大於 `created`，就壓到 `created`。缺 `cache_creation` 物件、`cache_creation:{}`、1h 欄為 null 或非數字時都填 None，代表未知（不是已知 5m）。其他 CLI 一律 None。

**回填**：`cli::UsageLog` 加 `usage_out: Option<&Mutex<Option<PromptCacheUsage>>>`，runner 在記帳那一處把解析結果寫進這個槽，作法同既有 `prompt_tokens_out`。
- 槽在 `run_turn` 的 loop 內每次嘗試新建，降級重開不會讀到失敗那次的數字。
- 只有 `Completed`＋抹寫成功的路徑讀這個槽、更新 lane。

**持久化**：`LaneState` 加 `cache_ttl_secs: u64`，`#[serde(default = "legacy_ttl")]`；舊 lanes.json 缺這欄時當 300。
- `run_turn` 呼叫前那次 `store.insert` 會整份重建 `LaneState`，所以先在 loop 外取出 `prior_ttl`。
- 呼叫前一律寫 0。中止、失敗、pending 未清的線下一輪本來就重開，不沿用。
- 成功後依下表覆寫（只有 claude 線；grok／agy 不回報快取時效，固定 300，維持原窗口）：

| 本輪結果（w1h＝1h 寫入、w5m＝created−w1h 飽和減） | `cache_ttl_secs` |
|---|---|
| 純 1h：w1h>0、w5m=0 | 3600 |
| 純 5m：w5m>0、w1h=0（含玩家 FORCE） | 300 |
| 混合：兩者都 >0 | 300（尾段 5m，不升 3600） |
| 有寫入但缺 breakdown（`created_1h` 為 None） | 300 |
| 純命中：寫入 0、讀取 >0，且本輪是同 session 的 Resume | 沿用 `prior_ttl` |
| 純命中但本輪是重開（新 session 撞到舊前綴，時效未知） | 300 |
| 讀寫雙零、或沒拿到 usage | 0（視為已過期，不延壽） |

以上整張表標〔模型判斷·未裁決〕。

**時鐘與過期**：時鐘基準維持呼叫開始時寫入的 `last_call_epoch`。
- `plan_turn` 判 `age > state.cache_ttl_secs`，取代固定 `CACHE_TTL_SECS`：300 秒不算過期，301 秒算。
- 過期時的行為照現況：**Resume 同一 session、整份換上新凍結 system、不送補丁（rebased）**，不重開。
- 停保溫後，閒置超過時效的線（例如久未發言的角色線）下輪就照這條追平；要是快取其實還活著，會多付一次寫入。

**診斷**：`LaneContext` 加 `ttl_secs`，記「呼叫前採用的 TTL」：Resume 取 `prior_ttl`，重開為 None。帳本列寫 `ttl_secs`。
- `usage/log.rs` 的 `short_reason`、`usage/report.rs` 的過期判定都改用列上的 `ttl_secs`；舊列缺這欄當 300。
- 不拿本輪新寫入的時效去解釋本輪是否過期。
- `CACHE_TTL_SECS` 改名 `LEGACY_CACHE_TTL_SECS`，只當舊資料預設值。

### 3. 帳本係數〔模型判斷·未裁決〕
帳本列寫 `created_1h_tokens`，只在有 breakdown 時寫。

**讀取係數按完整版本比對**（`report.rs` 的 `cache_price` 改吃 transport＋model）。比對要有版本邊界：id 等於版本本身，或版本後面接 `-`（例如日期後綴）才算；`claude-opus-5-50` 這類只是前綴相同的不算。

| 帳本 `model` 欄 | 讀取係數 | 估計？ |
|---|---|---|
| `claude-opus-5-5`（或 `claude-opus-5-5-…`） | 0.05 | 否 |
| `claude-fable-5-1`（或 `claude-fable-5-1-…`） | 0.025 | 否 |
| 別名 `opus` | 0.05 | 是 |
| 別名 `fable` | 0.025 | 是 |
| 其他（`claude-opus-4-6`／`4-7`／`5`、`claude-fable-5`、sonnet、haiku、別名 `sonnet`／`haiku`） | 0.1 | 否 |

別名的說明：
- 帳本存的是 `model_label`。CLI 別名現在指向 Opus 5.5／Fable 5.1，但舊列只有別名，看不出世代。
- 按現行別名套優惠屬估計，會改寫歷史省額（舊 Opus 別名列的省額會變大），所以這種列計入估計。

**寫入成本**：
- 列上有 `created_1h_tokens` 時＝`2.0×w1h + 1.25×w5m`。
- 舊列，或缺 breakdown（含 `cache_creation:{}`、1h 欄為 null 或非數字）的列，沿用 `1.25×created`，計入估計。

**估計標記**：`UsageRow` 加 `estimated_rounds`，模型列和總計都累計。分頁只有第一眼顯示省額（明細表沒有逐列省額欄），總計大於 0 時第一眼改用 `usageSavedHeadlineApprox`（「已省約 N% 費用」），十語系。

**歷史 ping 列**（`mode:"ping"`、`diag:"ping"`）：
- 花費同時加進總計和同 transport＋model 那一列的 `cost_usd`。
- 其餘一律不加：輪數、prompt、output、`observed_rounds`、`cache_rounds`、`cached_tokens`、`observed_prompt_tokens`、`saved_tokens`、`saved_usd`、快取分佈 `caches`。
- 現況 ping 列在扣合計前就進了分佈，這裡要改。
- 刪掉 `UsageTab` 181–199 行把 `report.ping` 加進第一眼合計的那段。

### 4. 超額提示
**超額是帳號狀態，不是桌的狀態：整個 app 執行期間只提示一次，不分桌**〔模型判斷·未裁決〕。

**解析**：`cli/stream.rs` 加 `claude_overage(line) -> Option<bool>`，讀 `rate_limit_event.rate_limit_info`。
- `isUsingOverage==true`，且 `overageStatus` 是 allowed 或 allowed_warning（或缺欄）→ `Some(true)`。
- `overageStatus=rejected` → `Some(false)`，不提示。

**收集，每次嘗試各自觀測**：`cli::UsageLog` 加 `overage_out: Option<&AtomicBool>`，槽在 `run_turn` 的 loop 內每次嘗試新建。
- runner 逐行讀到 EOF，result 前後的事件都檢查。
- 該嘗試內任一事件為 true 就記 true；後來變回 false 不撤銷。
- 帳本不記這個旗標。

**這次嘗試的寫入觀測**：嘗試結束時，同時讀這次嘗試的 `usage_out` 槽，歸成一種觀測：
- `OneHour`：純 1h。
- `Other`：純 5m、混合、缺 breakdown、零寫入。
- `Unknown`：沒拿到 usage，例如中止或失敗在 result 前。

**彙整**：`run_turn` 記下「第一個超額嘗試的觀測」`overage: Option<Observed>`，不跟其他嘗試的時效混搭。
- 例：第一次超額、純 1h、續聊失敗，第二次非超額、純 5m、成功 → 回報第一次的 `OneHour`。

**回報**：`LaneCall.on_overage` 回呼，在第一個超額嘗試結束當下呼叫一次，帶那次嘗試的觀測（`lanes::CacheWriteObserved`）。整輪後來中止或 `Err` 都已報過——錢已經花了，照樣提示〔模型判斷·未裁決〕。用回呼而非改 `run_turn` 回傳型別，錯誤路徑不必另帶資料。

**發送**：`commands/chat.rs` 的 `prepare_play_lane_call`（角色線與 GM 線共用）組 call 時一併釘 1h、掛回呼，回呼 emit `claude-overage`，payload 為 `{ eventId, observed: "one-hour" | "other" | "unknown" }`，不帶 world 或 turn。

**前端**：仿 `SmartFreeNoticeToast` 新增 `ClaudeOverageToast`（不自動消失，玩家關掉為止；文案在 `i18n/features/claude-overage.ts`），掛在 `App.tsx`，跟 app 生命週期走，不掛在會卸載的 `useChatController`。
- 一次性旗標放在模組層級，app 重啟才清空。在事件處理當下、顯示之前同步設置，避免連續事件重複顯示。
- 沿用 smart-free 的 `disposed` 檢查，處理非同步 `listen` 註冊和卸載之間的空窗：註冊完成前就卸載，要立即解除監聽。
- 已卸載（`disposed`）就直接返回、不設旗標，所以卸載期間到達的事件不會耗掉名額。
- 哪一桌觸發都在當下顯示。純資訊，沒有動作按鈕。

**文案**（十語系，三種）：
- `one-hour`：「Claude 訂閱額度已用完，這輪起按超額計費；這輪的快取以 1 小時寫入，每次寫入約為一般輸入的 2 倍價格。」
- `other`：「Claude 訂閱額度已用完，這輪起按超額計費；快取寫入按實際時效計價（5 分鐘約 1.25 倍、1 小時約 2 倍）。」
- `unknown`：「Claude 訂閱額度已用完，這輪起按超額計費。」不寫倍數。

不從 usage 推斷「是你的設定強制 5 分鐘」。

### 5. 撤保溫與離開提醒（不留死碼）
**後端**：
- `lanes::keepalive`、`PING_PROMPT`、`PING_MIN_AGE_SECS`、`restore_before_ping`。
- `session_file::appended_since`、`marker_user_lines` 和它們的測試。
- 指令 `keepalive_lanes`：`commands/chat.rs`、`lib.rs` 的註冊與 `WRITE_WORLD` 清單。
- `LaneContext.ping`、`Mode::Ping`、`UsageReport.ping` 與 `is_ping` 累計、`ping-truncate-failed` 原因。
- 舊帳本 ping 列的讀取照第 3 點。

**測試**：
- 假 CLI 的保溫分支、`keepalive_*` 測試、`ping_with_fake_mode`。
- `lanes/tests/lane_lock.rs` 改寫，見「四、驗收」。

**`lane_lock`**：保留（`run_turn` 從讀檔到 CLI 結束都持有它），只改模組和函式註解。

**前端保溫**：
- `useChatController.ts` 的 `KEEPALIVE_*`、計時器、`lastTurnAt`、`pingCount`、`keepaliveOff`、`awayTooLong` 狀態、`noteTurnDone` 和它在 controller 內的四處呼叫。
- 回傳型別上的 `awayTooLong` 和 `noteTurnDone`。

**離開提醒**：
- `useSceneActions.ts` 的 `noteTurnDone` 介面欄位和兩處呼叫。
- `PlayView.tsx` 的 `awayTooLong` prop、`SCENE_AWAY_HINT_MIN_CHARS`、`showAwayHint` 和顯示段。`sceneTooLongHint` 保留。
- `AppWorkspace.tsx:350` 的 prop 傳遞。
- 十語系 `sceneAwayHint`。

**UI 與 i18n**：`UsageTab` 的 ping 列、`.usage-ping-row`，以及 i18n `usagePing`、`usageModePing`、`usageReasonPingTruncateFailed`。

**前端測試**：
- `keepalive_lanes` mock：`app-navigation.test.tsx`、`refactor-undo-flow.test.tsx`。
- `awayTooLong={false}`：`PlayView.test.tsx`、`chat-smart-free-notices.test.tsx`。
- `noteTurnDone`：`scene-actions-lock.test.tsx`。

**文件**：
- 撤掉實測佇列第 23 項。
- 改掉 `session_file.rs`、`lanes/mod.rs` 模組註解裡的保溫字樣。

## 四、驗收

**Rust 單元／假 CLI**：
- 假 CLI 吐出以下各種 usage，核對 lanes.json 的 `cache_ttl_secs` 符合第 2 點的表：
  - 純 1h → 3600。
  - 純 5m → 300，且帳本寫入係數用 1.25。
  - 混合 → 300。
  - 零寫入純命中，續聊時沿用、重開時為 300。
  - 讀寫雙零 → 0。
  - 缺 breakdown，包括整個 `cache_creation` 缺欄、`cache_creation:{}`、1h 欄為 null、1h 欄非數字，都當未知 → 300，且該列標估計；不得當成已知 5m。
- 邊界：
  - `cache_ttl_secs=300` 時，age 300 → patch 續聊；age 301 → rebased Resume，不重開、session id 不變。
  - `cache_ttl_secs=3600` 時，age 301 → patch 續聊；age 3601 → rebased。
- 舊 lanes.json 缺欄讀成 300。寫檔後重新讀檔，`cache_ttl_secs` 仍保存。
- 降級重開：第一次嘗試失敗、第二次純 5m → 結果為 300，不沿用第一次的數字。
- `created_1h > created` 時飽和成 created、不下溢。舊帳本列（無 `created_1h_tokens`、無 `ttl_secs`、含 ping 列）的報表數字：ping 只進總計和同 transport＋model 列的 `cost_usd`，輪數、token、`observed_rounds`、快取分佈、命中率、省額都不變；讀取係數：
  - `claude-opus-5-5…` 0.05、`claude-fable-5-1…` 0.025，非估計。
  - 反例 `claude-opus-4-6`、`claude-opus-4-7`、`claude-fable-5`、`claude-opus-5-50`（只有前綴相同）皆 0.1。
  - `claude-opus-5-5-20261001`（日期後綴）仍算 0.05。
  - 別名 `opus`／`fable` 套 0.05／0.025 且計入 `estimated_rounds`；別名 `sonnet` 0.1，非估計。
  - 有估計列時，模型列和總計的省額都帶「約」標記；全為新列時不帶。
- 環境變數：
  - 續聊線 `LaneCall.envs` 含 `CLAUDE_CODE_PROMPT_CACHE_TTL=1h` 和 `FORCE_PROMPT_CACHING_5M=0`。
  - 單發 `stream_via_transport` 的 claude 分支與卡重構 `refactor_ai/session.rs` 的 open／resume 都只用共用的 `claude_cli_envs`（含相容端點設定時），單元測試核對它沒有這兩個變數；釘 1h 只發生在 `prepare_play_lane_call`。
- 超額：假 CLI 分別吐出以下事件，核對 `on_overage` 收到的觀測（Some＝報一次、None＝沒報）：
  - `isUsingOverage:true` 在 result 前、在 result 後 → Some。
  - true→false → Some。
  - `overageStatus:rejected` → None。
  - 純 1h → `OneHour`；純 5m、混合、缺 breakdown → `Other`；中止在 result 前 → `Unknown`。
  - 反例：第一次超額、純 1h、續聊失敗，第二次非超額、純 5m、成功 → `OneHour`，不是 `Other`。
  - 整輪 `Err`、但中間嘗試超額 → 錯誤回傳前仍 emit。
- 同桌並發：
  - 假 CLI 用可控阻塞點：啟動後寫「已開始」標記檔，等釋放檔出現才繼續。
  - A 線阻塞時啟動 B 線：確認 B 的假 CLI 沒有開始；放行 A 後 B 才跑。
  - 結束後 lanes.json 兩條線都在，session id、`sent_hash` 各自正確。
  - A 失敗或被取消後，B 仍拿得到鎖並完成。
  - 不用 sleep 賭時序。

**前端**：
- 假計時器推進 10 分鐘，零次 `keepalive_lanes` invoke。
- `command_classification` 同步測試確認後端已無這個註冊。
- 超額事件（`ClaudeOverageToast`）：
  - 多桌、多次超額事件只顯示一次；換桌、離桌不重跳。
  - 同一個 tick 內連續送兩個事件只顯示一次（旗標同步設置）。
  - `listen` 延遲註冊期間元件卸載 → 不留監聽、不顯示、不耗名額。
  - 重新掛載後仍能收到下一個事件並顯示（名額未被耗掉時），不會重複顯示。
  - 三種 `observed` 各對應正確文案；`unknown` 不含倍數。

**`npm run verify` 全綠。**

**測試通道實測（claude Sonnet，凍結場景和卡）**：
- 新桌第一輪：帳本列 `created_1h_tokens>0`、`ttl_secs` 為 None，lanes.json 為 3600。
- 等超過 5 分鐘（約 6 分鐘），不改任何東西，送同一角色第二輪：
  - session id 不變、plan 是 patch 續聊而不是 rebased。
  - `cached ≥ 0.9×expected_cached`（沿用既有九成標準）。
  - AI 紀錄沒有任何保溫呼叫。
- 這只證明「超過 5 分鐘仍命中」，不驗完整一小時邊界；1h 帳桶以 `ephemeral_1h_input_tokens` 為準。
- 離開提醒和 ping 列不再出現。

**實測結果（2026-10-07，測試通道、Sonnet、範例桌狐狸線、凍結場景與卡）**：
- 第一輪開線：帳本 `created_1h_tokens=1607`＝全部寫入、無 `ttl_secs`，lanes.json `cache_ttl_secs=3600`。
- 閒置 444 秒後第二輪：同 session、一般續聊（非 rebased）、帳本 `ttl_secs=3600`，讀到 527。第一輪讀 0，所以這 527 只能來自第一輪寫的快取；5 分鐘快取到這時已過期。
- **「cached ≥ 0.9×expected_cached」未達**（第二輪 527／1609）。原因不在本案：CLI `--resume` 只重用 system 那段，前一輪對話尾端每輪重寫。對照：秒送的第三輪同樣只讀 527；直接跑 CLI 開線＋兩次 resume，帶與不帶 1h 環境變數讀寫都一樣（7,839／7,840，每輪尾端重寫）。範例桌 system 小，所以命中率低、分頁亮「該中沒中」。CLI resume 尾端重寫是否另立案，由主線問作者。
- AI 紀錄 3 次派送對 3 輪，等待期間零保溫呼叫；額度分頁沒有 ping 列，全新紀錄標題不帶「約」。

**明列未驗**（需實機或外部條件的已排進實測佇列梯 3）：
- 真超額（本帳號 `org_level_disabled`）。
- 玩家 `~/.claude/settings.json` 的 `env` FORCE 在 `--safe-mode` 下是否生效。
- 一小時邊界實跑。
- Windows。
