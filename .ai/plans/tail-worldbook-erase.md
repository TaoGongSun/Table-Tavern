# tail-worldbook-erase 方案

## 一、裁決
- GM 續聊線與角色共線的尾段世界書（本輪觸發的 keyword、機率、計時、作者註記、依深度等）**累積到門檻再一次清乾淨**：舊尾段先留在 session 歷史，累積到一定量才整段清一次；模型可能看到舊條目，但份數有上限；Claude、Grok、Agy 都套同一套〔作者裁決 2026-10-11，取代「每輪抹」〕。
- 作者註記與依深度一律放回合尾段（P2）〔作者裁決 2026-10-10〕：本案任何路徑都不把它們搬進 system。
- `Hoist::All` 線（Agy 一角一線、單人在場 Claude）維持現行：本輪世界書在 system、變了就重開，不在本案〔作者裁決 2026-10-11，見 worldbook-st-trigger-parity.md 七〕。
- GM 尾段的「目前狀態」與觸發器不抹、不計入門檻（本案只處理世界書）。
- 背景（三方實查）：Claude CLI 每個 request 只留最後一個斷點可重用，每輪抹最新 user 行＝下一輪只中 system、歷史每輪以寫入價重寫（claude-resume-tail-cache.md 實測，第 4 輪寫入約為不抹時的 3.4 倍）；TestCards 32 張裡 15 張有常駐作者註記／依深度條目，從第 1 輪起每輪觸發；換幕本來就重開，疊加最多一幕。

## 二、現況（掛點）
- 角色共線尾段（`transport/turns.rs` `chars_lane_turn`，`Hoist::None`）：`public`（本輪公開觸發、不在共用快照的條目，含注入段）不抹，每輪疊一份；`confidential`（動態公開設定、私設、限定條目、狀態）每輪抹。
- GM 線尾段（`gm_lane_turn` → `state_view.rs` `gm_dynamic_block`）：世界書段＋目前狀態＋觸發器＋導演指示，三家都不抹。
- 重開（`lanes/mod.rs` `plan_turn` → `TurnPlan::Reopen`）：新 session，prompt＝對話紀錄標頭＋本幕全部事件＋本輪 tail（`build_prompt`，`opening`）。舊回合的 tail（世界書、狀態、補丁、導演指示）不是事件，重開後全部消失。舊線處置：Grok 撤線刪目錄；Claude GM 線 `retire_claude_gm_session` 刪舊檔；Claude 角色線與 Agy 不刪（舊 id 不再續用）。
- 容量上限：`scene_budget::path_limits` 的 `budget`（`Budget { unit, max: total − reserve }`），聊天與 GM 組裝時已經算好拿去做世界書預算。Claude 預設 200,000 − 12,288 ＝ 187,712 tokens；Grok 204,800 − 12,288；Agy 以 bytes 計 190,000。

## 三、做法

### 1. 清法：到門檻排一次重開（a）
- **(a) 到門檻排一次 `Reopen`**（採用）：沿用既有重開，GM／共線、Claude／Grok／Agy 同一條路，不需新的抹寫碼。
- **(b) 到門檻一次抹掉歷史裡所有舊尾段**（不採用）：要寫跨多則 user 行的 Claude jsonl 抹寫、Grok 兩檔的多回合抹寫，Agy 沒有抹寫路徑仍得退回 (a)；快取代價與 (a) 同級。
- (a) 的附帶效果：重開把整段舊 tail 一起壓平——GM 歷史裡每輪的「目前狀態」與觸發器、Claude 角色線的舊補丁也一起消失，重開後只剩本輪那份。
- 本案不新增、不修改任何抹寫路徑；角色共線的機密段照舊每輪抹（隔離用）。

### 2. 門檻〔模型判斷·未裁決〕
- **計量對象**：本輪尾段世界書的文字，組裝端以 `LaneTurn.tail_worldbook: Option<String>` 交出（GM＝世界書段，含段標；角色共線＝`public` 段）；`Hoist::All` 與沒有尾段世界書時為 None。這段不在機密段裡，回合後原文留在歷史，所以估它就是估「收尾後留下的世界書文字」。大小依容量單位算：tokens 用 `scene_budget::budget_tokens`，bytes 用 UTF-8 長度。這是文字估計，不含 thinking、reasoning、CLI 附加內容；Grok 兩檔存同一段，計一次。
- **分母**：該路徑的固定容量 `Budget.max`（total − reserve），經 `TurnInput.capacity: Option<Budget>` 帶入 `plan_turn`。實送（`chat_with_character`、`gm_lane_reply`）用組裝時同一個 `path.budget`。上限解析不出來（None）時只看份數上限。
- **觸發條件**（滿足任一）：
  1. 份數上限：`copies >= 8`。
  2. 比例：`copies >= 4` 且 `stale >= capacity × 15%`（最少 4 份〔作者裁決 2026-10-11〕）。
- 不做「快取過期順手清」〔模型判斷·未裁決〕：`cache_ttl_secs` 在 Claude 缺用量或讀寫皆 0 時是 0（會每輪誤觸發）、Grok／Agy 是寫死的 300 秒佔位值；Claude 重開也比冷續聊多一次「開線後第 2 輪不中」，並不免費。
- **為什麼 15%**：比例只防大條目把 context 撐大；分母是固定容量，重開後不會因歷史變短而每輪重觸發。以 Claude 預設容量算，15% ≈ 28.2k tokens；以 `budget_tokens` 估，orc-cave 每份約 10.8k、HeroTraining 約 17.5k，兩張都在第 3 份就過 15%，由最少 4 份擋到第 4 份。最少 4 份保證兩次重開之間至少隔 4 輪（大條目卡 4 輪中 2 輪命中歷史快取、舊尾段最多 3 份），份數上限 8 保證小條目也有上限。三個數值都是 `lanes` 內的常數。
- **存放**：`LaneState.tail_stale: Option<TailStale>`，`TailStale { copies: u32, tokens: u64, bytes: u64 }`，`#[serde(default)]`。
  - **None＝未知**（升級前的舊 `lanes.json`）：會帶尾段世界書的線在其他重開原因都不成立、要續聊時，以 `tail-worldbook` 重開一次。判定看線的形狀：`TurnInput.tail_tracked`＝GM 線，或角色線且 `!shape.hoist_private`；`Hoist::All` 線不受影響。新建的狀態一律寫 `Some`，不會出現 None 被當 0 的誤判。
- **帶入與累加**（`run_turn`）：
  - 每次嘗試前 `store.insert` 新狀態：計畫是 `Resume` 就從 prior 帶入 `tail_stale`；是 `Reopen`（含同輪續聊失敗降級的 `ResumeFailed`）就歸零。
  - 只在成功收尾、最終落檔時累加本輪那份（有 `tail_worldbook` 才 `copies += 1`）。中止、失控、空回覆、Agy 沒拿到 conversation id、抹寫失敗丟線都不累加（下一輪本來就重開或 pending）。
- **帳本**：`LaneContext` 加 `stale_copies`、`stale_size`、`stale_capacity`、`stale_ratio`（呼叫前的值；重開那行也記清理前的值，狀態另外歸零），重開原因 `tail-worldbook`。capacity None 時 `stale_capacity`、`stale_ratio` 記 null（未知，不是 0%）。

### 3. 插入點（`plan_turn`）
- 保留既有重開原因的先後：首輪、換 CLI、pending、換幕、收回、改動、回覆分岔、未抹線三項、`worldbook-changed` 都在前。
- GM 線與非 Claude（`:700` 起）：先判 `SystemChanged`，再判門檻（含未知），最後 `Resume`。
- Claude 角色線：TTL 過期的 rebased `Resume` 與快取存活的補丁 `Resume` 之前各判一次門檻。
- 判斷集中成一個 `tail_reopen(state, input) -> bool`，三個出口共用。

### 4. 各線與舊 session
- Claude／Grok GM 線、Claude／Grok 角色共線、Agy GM 線：照上面計數與重開；尾段世界書照舊在尾段。
- Claude 角色線因 `tail-worldbook` 重開時也刪舊 session 檔：`retire_claude_gm_session` 改名成不分線的 `retire_claude_session`（落檔成功才刪、刪不掉記 `cleanup-failed`、落檔失敗回錯）；角色線其他重開原因照舊〔模型判斷·未裁決〕。Grok 照舊撤線；Agy 舊對話留在 Agy 端（現況，無刪除路徑）。

### 5. 快取、水位、容量
- **快取**：本案改善的是 GM 線與共線公開段的堆疊——門檻內這兩種內容不再造成抹寫，GM 線續聊照舊中到上一則 user（Claude）／上一截（Grok）。共線仍每輪抹機密段，命中情況與現在相同。到門檻那輪是重開：Claude 歷史整段以寫入價重寫一次，加上既有的「開線後第 2 輪 CLI 端不中」一次；Grok 撤線重建；Agy 開新對話。
- **水位**：重開本來就從 0 起算，續聊不改。
- **容量量測**：不改碼；限制見七。

## 四、測試清單
- `transport/turns.rs`：
  - `tail_worldbook` 與 tail 內實際那段逐字相同（GM、角色共線各一）；沒有尾段世界書時 None。
  - `Hoist::All`（chars_lane_turn 帶 `Hoist::All`）時 `tail_worldbook` 必為 None。
  - tail、system 逐字不變。
- `scaffold_baseline` 三語系：預期完全不變；有差異照實寫並逐段核對。
- `plan_turn` 單元（直接造 `LaneState`，固定 capacity）：
  - 份數：小條目 `copies` 7 → 續聊、8 → `tail-worldbook`（固定 capacity 夠大，保證比例先不觸發）。
  - 比例：`copies` 3 即使超過 15% 仍續聊；`copies` 4 時 15% 剛好與差 1 的兩側。
  - capacity None：只看份數。
  - 同時成立：`SystemChanged`＋門檻 → 記 `system-changed`；TTL 過期＋門檻（Claude 角色線）→ 記 `tail-worldbook`；首輪、換幕、pending、收回、改動、回覆分岔、`worldbook-changed`、未抹線三項各與門檻同時成立 → 記原本那個原因。
  - 舊 store 升級：`tail_stale` None 的 GM 線與 `Hoist::None` 角色線 → `tail-worldbook` 重開一次、之後照常；`tail_tracked` false 的線（`Hoist::All`）None → 照常續聊；舊 `lanes.json` 文字照讀不壞。
- `lanes/tests`（假 claude）新檔 `tail_threshold.rs`：
  - GM 線每輪固定小作者註記跑 10 輪：第 9 輪 `reopen: tail-worldbook`、其餘（首輪外）續聊；**每輪結束後 session jsonl 的 user 行裡世界書段標出現次數 ≤ 8**（只數 user 行，排除 system 裡同名的 `known_world_heading` 與 GM 世界書段標）；重開那輪 prompt 只含本輪一份、舊 session 檔已刪。
  - 大條目（單份超過 capacity 的 15%）跑 20 輪：兩次重開間隔都 ≥ 4 輪。
  - 角色共線跑到門檻：舊檔已刪、機密段照舊每輪抹、大機密段不影響計量（只計 `public`）。
  - 續聊失敗降級（`ResumeFailed`）那輪：計數只剩本輪一份。
  - 中止、失控：計數不增。
  - 重開時 store 落檔失敗：回錯、舊檔不刪；刪檔失敗：記 `cleanup-failed`、本輪照常。
  - 帳本：`stale_copies`／`stale_size`／`stale_capacity`／`stale_ratio` 寫進續聊行與 `tail-worldbook` 重開行（清理前的值）；capacity None 時後兩欄 null。
- Grok（假 grok）：GM 線到門檻撤線重開、計數歸零。Agy（假 agy）：GM 線到門檻開新對話、尾段世界書仍在 prompt 尾段、system 不含；容量以 bytes 計。

## 五、驗收
- `npm run verify` 全綠；不留 tsc 產生的未追蹤檔。
- 測試通道（claude 低階檔位，容量 187,712 tokens，15% ≈ 28.2k）：
  - `TestCards/orc-cave-copy.png`（尾段世界書每份約 10.8k）與 `TestCards/HeroTrainingUnderSide_stats.png`（約 17.5k）：都預期第 5、9…輪 `tail-worldbook` 重開（第 3 份已過 15%，被最少 4 份擋到第 4 份）。
  - 實卡的觸發量每輪不同，不保證精確輪數；推進到至少觸發一次門檻，逐輪記帳本快取讀／寫量、`stale_copies`、`stale_ratio`、`reopen`，每輪結束後數 session jsonl user 行的世界書段數（≤ 8）、重開後只剩一份、舊檔已刪，照實記錄與預期的差。
- Grok GM（低階檔位）若驗：同樣跑到門檻，記每輪讀量與 `reopen`。
- 文件同步：`worldbook-st-trigger-parity.md` 七「GM 續聊線與角色共線的公開段在回合後不抹」改成到門檻重開；`lanes/mod.rs` 模組註解與 `plan_turn` 註解。

## 六、範圍外（另案）
- 週期性重開後，「開線後第 2 輪 CLI 端不中」會在帳本週期性標成 `below-expected`；本案不改帳本判定。

## 七、已知差異與限制
- 門檻內模型看得到最多八份舊注入（ST 每次生成只送本輪觸發的條目）。
- 大條目卡（單份超過容量 15% 的四分之一）每 4 輪重開一次，Claude 4 輪中 2 輪命中歷史快取（重開那輪與開線後第 2 輪不中）。
- Agy 的分母 190KB 是單則 body 上限；續聊時舊尾段在伺服器端歷史，不在本輪 body，所以比例只是代理指標。
- 容量解析不出來（None）時只保證份數有界，不保證總大小。
- 多卡共線變成單人在場（改走 `Hoist::All`）、且該輪提進 system 的世界書為空時，世界書指紋前後都是 None、不觸發 `worldbook-changed`，多卡時期留在歷史的舊公開段要等別的原因重開才清（極少見，不改）〔模型判斷·未裁決〕。
- 容量量測只算一份尾段，門檻內實際會多出舊尾段；本案不改量測。Agy 容量以 bytes 計，比例也以 bytes 算。
- 週期性重開的帳本誤報見六。

## 八、施工結果
- 實作：`lanes/mod.rs`（`TailStale`、三個常數、`tail_reopen`、`ReopenReason::TailWorldbook`、計數帶入與累加、`retire_claude_session` 角色線到門檻也清）、`transport/turns.rs`（`LaneTurn::tail_worldbook`）、`chat_assembly::gm_lane_parts` 回 `LaneTurn`、`commands/chat.rs`（`tail_tracked`＝GM 或 `!shape.hoist_private`、`capacity`＝`path.budget`）、`usage/log.rs`（`StaleTail` 四欄）。tail、system 逐字不變，scaffold baseline 未變。
- 測試：`lanes/tests/tail_threshold.rs`（11）、`lanes/tests/grok.rs` GM 到門檻、`transport/turns.rs` 尾段世界書逐字與 `Hoist::All` 為 None。
- verify 12 步綠；cargo test 1579、vitest 1141、web vitest 774。
- 測試通道（claude-haiku-4-5，容量 187,712）：orc-cave 與 HeroTraining 是露骨成人卡（前者非合意題材），不拿來產生內容，改用同樣有常駐依深度條目的 `WestFantasy_NOUI.png`（尾段世界書每份約 0.9–1.4k tokens，預期份數上限先到、第 9 輪重開）。GM 十輪：

| 輪 | reopen | 快取讀 | 快取寫 | 呼叫前份數 | 佔比% | session 內世界書份數 |
|---|---|---|---|---|---|---|
| 1 | first-turn | 0 | 8,219 | — | — | 1 |
| 2 | — | 8,219 | 2,705 | 1 | 0.5 | 2 |
| 3 | — | 10,924 | 2,654 | 2 | 0.9 | 3 |
| 4 | — | 13,578 | 2,936 | 3 | 1.4 | 4 |
| 5 | — | 16,514 | 2,832 | 4 | 1.9 | 5 |
| 6 | — | 19,346 | 3,089 | 5 | 2.3 | 6 |
| 7 | — | 22,435 | 3,318 | 6 | 2.8 | 7 |
| 8 | — | 25,753 | 4,438 | 7 | 3.3 | 8 |
| 9 | tail-worldbook | 6,728 | 15,776 | 8 | 3.8 | 1 |
| 10 | — | 22,504 | 5,234 | 1 | 0.7 | 2 |

  與預期一致：第 9 輪重開、session 份數不超過 8、舊 session 檔已刪、無 cleanup／rewrite 失敗。第 10 輪（開線後第 2 輪）這次完整命中，沒有出現既有的一次性不中。
- 比例門檻實機（claude-haiku-4-5，空桌自建一條約 8k tokens 的常駐依深度地理志條目，合成文字）：

| 輪 | reopen | 快取讀 | 快取寫 | stale_copies | stale_size | stale_capacity | stale_ratio% | session 內世界書份數 |
|---|---|---|---|---|---|---|---|---|
| 1 | first-turn | 0 | 8,300 | — | — | — | — | 1 |
| 2 | — | 8,300 | 8,447 | 1 | 8,032 | 187,712 | 4.3 | 2 |
| 3 | — | 16,747 | 8,459 | 2 | 16,064 | 187,712 | 8.6 | 3 |
| 4 | — | 25,206 | 8,398 | 3 | 24,096 | 187,712 | 12.8 | 4 |
| 5 | tail-worldbook | 0 | 10,556 | 4 | 32,128 | 187,712 | 17.1 | 1 |
| 6 | — | 10,556 | 9,084 | 1 | 8,032 | 187,712 | 4.3 | 2 |

  與預期一致：第 3 份（12.8%）未達 15%，第 4 份（17.1%）觸發，第 5 輪重開；舊 session 檔已刪、無 cleanup／rewrite 失敗。重開那輪讀 0 是因為空桌 system 只有約 250 tokens，低於 haiku 的快取門檻。
