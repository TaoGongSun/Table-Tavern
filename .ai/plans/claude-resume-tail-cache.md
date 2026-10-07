# claude-resume-tail-cache 規格

由 [handoffs/archive/claude-resume-tail-cache.md](../handoffs/archive/claude-resume-tail-cache.md) 連回。

## 一、查證結論（claude 2.1.287，Sonnet 實送）

「續聊只命中 system 段」有三個成因：兩個在 CLI 端、一個在 app 端。

1. **CLI：`total_tokens_reminder` 在 live 與 resume 之間不一致**（主因之一）。
   - CLI 每輪都把 `<total_tokens>N tokens left</total_tokens>` 存成 session 檔裡的 attachment 行（模式由 GrowthBook 下發，預設 `padded-countdown`，預算 15,000,000）。
   - 即時送出的 request 不帶這段；`--resume` 重組歷史時卻會渲染它。第一輪那筆併進 `[1]` Environment system 訊息，之後每輪各變成一則獨立的 `role:"system"` 訊息，接在該輪 user 後面。
   - 結果：resume 送出的歷史前綴，從第 `[1]` 則開始就和上一輪寫入的不同。
   - 開關：環境變數 `CLAUDE_CODE_TOTAL_TOKENS_REMINDER=off`（CLI 內標 @internal）。
2. **CLI：開線那輪寫的快取，第二輪 resume 用不到**（成因不明，只損失一次）。
   - 已排除的可能：
     - 同一程序內連送能讀到開線那輪寫的快取。
     - 兩次相同的開線也能互相讀到。
     - 假端點下，開線與 resume 的 body（只差 `cache_control` 位置，以及 str／單一 text 區塊的形狀）和 header（只差 session id）都相同。
   - 推測：差異只在真實環境出現，可能和伺服器下發的旗標有關。
   - 未能直接抓真實 request：用 `ANTHROPIC_LOG` 記錄的路被權限分類器擋下（憑證外洩風險），所以沒再走。
3. **app：回合後抹掉最新 user 行的機密段，等於作廢唯一可用的快取項**（主因之二）。
   - 實測：每個 request 只有最後一則訊息的斷點會留下可重用的快取項。CLI 雖然也在倒數第二則（assistant）放了斷點，分支實驗卻讀不到那一段。
   - 機密段＝私設、限定世界書條目、角色狀態區塊。claude 角色線沒有提私設（`hoist` 只給 agy），所以卡片只要有私設、觸發限定條目或有狀態欄，就會每輪都抹。
   - 抹完後，下一輪的前綴在上一則 user 就分岔，往前也沒有其他快取項可用，只剩 system 段命中。
   - 補「名字：」前綴改的是上一輪的回覆，那段本來就不在任何快取項裡，所以**不影響命中**。

lanes/mod.rs 模組註解寫的「命中率天花板＝只有最後一句沒中（E6：99.7%）」在現行 CLI 下不成立，施工時一併改掉。

### 實測數字

條件：system 約 11.1k token、每輪 user 約 4.4k；用乾淨環境，比照 app 只帶 `CLAUDE_CODE_SANDBOXED=1` 與兩個 1h 變數。每格格式為 cache read／cache creation。

| 情境 | 第 1 輪 | 第 2 輪 | 第 3 輪 | 第 4 輪 |
|---|---|---|---|---|
| resume，現狀（提醒開） | 0／15,725 | 11,099／9,049 | 11,099／12,755 | — |
| 同程序連送（對照） | 0／15,726 | 15,726／4,428 | 20,154／3,694 | — |
| resume，提醒 off | 0／15,704 | 11,101／9,014 | **20,115／3,662** | **23,777／3,748** |
| resume，提醒 off，每輪抹機密段＋補前綴（app 現狀） | 0／15,704 | 11,100／8,981 | 11,100／12,632 | 11,100／16,340 |

用提醒 off 那條線的真 session 做分支實驗（截到第 3 輪後再 resume 第 4 輪）：

| 分支 | 第 4 輪 |
|---|---|
| 只抹最後一則 user | read 20,115，退到上上輪的最後斷點 |
| 只補前綴 | read 23,777，完整命中 |

## 二、裁決

1. claude CLI 呼叫一律帶 `CLAUDE_CODE_TOTAL_TOKENS_REMINDER=off`。理由：不帶的話，連沒有機密段的線也只命中 system 段。〔作者裁決 2026-10-07〕
2. 單角色線不抹機密段。理由：ST 單角色卡是大宗。〔作者裁決 2026-10-07〕
3. claude 單角色線比照 agy，把私設提進凍結 system。理由：私設不必每輪重複留在歷史。〔作者裁決 2026-10-07〕
4. session 檔出現 `total_tokens_reminder` attachment 時只記診斷，不剝除。理由：抹寫路徑維持只改 user／assistant。〔作者裁決 2026-10-07〕

已知限制：開線後第 2 輪會把開線內容重寫一次（第一節第 2 點，成因在 CLI 端）。CLI 升級後再用第一節的方法實測。

## 三、施工

**A. 關提醒**
- 環境變數加在共用的 `claude_cli_envs`（`transport/dispatch.rs`）。
- `prepare_lane_call`（遊玩各線、卡重構）與單發呼叫都經過它，單發帶了也無害。

**B. 單角色線（隔離規格）**

隔離只靠一條規則：**含某角色未抹內容的 session，只有該角色、在單角色模式下才能續用**。在場人數只決定模式，也就是只影響成本；隔離不靠人數來保證。

1. **模式判定（呼叫端組素材時決定）**
   - 有效在場集合＝未封存的卡中，「非 auto_hidden」或「本幕已回歸」（`data::appeared_card_names(events)` 以名字比對）的那些，再加上本輪開口者本人（即使他不在 cards 裡）。
   - 只有集合恰好等於 {開口者} 時是單角色模式；零卡時集合只有開口者，也算單角色。
   - 名字比對造成的多算只會讓判定偏向多角色，寧可多抹。
   - chat.rs 與 `scene_budget/measure.rs` 共用同一個判定函式，讓容量估算與實送一致。
   - measure.rs 現在 grok 也算 hoist，而且 `turn.tail` 已經含機密段、它又在前面再接一次。施工時一併改成照實送組裝。
2. **三個開關拆開**（`commands/chat.rs:145` 現在一個 hoist 同時管三件事）

   | 線 | 私設搬進 system | 傳 confidential（抹） | 補名字前綴 | lane key 分角色 |
   |---|---|---|---|---|
   | agy | 是 | 否 | 否 | 是 |
   | claude 單角色 | 是 | 否 | 是 | 否 |
   | claude 多角色 | 否 | 是 | 是 | 否 |
   | grok | 否 | 是 | 是 | 否 |

   - grok 不套單角色模式，維持每輪抹。理由：作者裁決只涵蓋 claude，grok 的 system 凍在開線那刻。〔作者裁決 2026-10-07〕
   - hoist 只搬 `private_md`。限定條目與狀態區塊仍留在 tail，算「未抹歷史」。
3. **記錄**
   - `LaneState` 新增 `unerased_owner: Option<String>`（角色 id），`#[serde(default)]`。
   - 舊 lanes.json 缺這個欄位時讀成 None，等於照舊每輪抹；整份 store 照常反序列化。
   - 只有 **claude 角色線**會寫 owner；agy、grok、GM 一律寫 None（`had_state_block` 也一律 false），所以 B-4 第 2～4 項不會套到它們。agy 的隔離靠分角色的 lane key，不靠這兩個欄位。
   - claude 角色線每次嘗試在呼叫前寫 store 時一併寫入：
     - 單角色模式（開線或續用皆同）：寫開口者 id，私設是空的也一樣寫；前線 owner 為 None 而續用時也是在這一刻記下新 owner；
     - 多角色模式（開線，或續用 owner 為 None 的既有抹寫線）：寫 None。
   - owner 為 None 的多角色線照舊每輪 Resume＋抹寫，不因本案重開。
   - `pending_rewrite` 照舊每次嘗試前寫入。單角色模式雖然沒有 confidential，仍有 prefix，崩潰保護不變。
   - 成功收尾時只清 `pending_rewrite`，`unerased_owner` 保留到 session 被換掉為止。
4. **鎖內重判**（`plan_turn`，本來就在 lane 鎖內）。判定順序固定，前面命中就不再往下：
   1. 既有條件：provider 不同、pending、換幕、正典收回、已送段被改、回覆分岔；
   2. `OwnerChanged`：前線 owner＝X 且本輪開口者不是 X，包括換人，以及別的角色解析到同一模型而共用同一 key；
   3. `ModeChanged`：前線 owner＝X 且本輪是多角色模式，也就是在場變多，即使開口的仍是 X；
   4. `StateBlockGone`：前線 owner＝X、上輪有狀態區塊、本輪沒有（見 C）；
   5. 以上都沒命中才進 claude 原本的 Resume／補丁／rebased 判斷。
   - 第 2～4 項只在前線 owner 有值時成立；owner 為 None 的線一律跳過，照原規則續用。
   - 同一個 X 在單角色模式下回來，且第 4 項沒命中，就照常續用、不丟線。
   - resume 失敗時，降級重開沿用同一迴圈；每次嘗試前都依本輪模式重寫 owner。
   - 中止、runaway、抹寫或前綴改寫失敗、開線失敗的收尾維持現況：留 pending 或刪 key，下一輪重開。〔作者裁決 2026-10-07〕
5. **私設變動**
   - 沿用現有補丁機制（`plan_turn` 的 claude 分支）：
     - 快取未過期時，送舊 snapshot，差異以補丁貼在本輪 user；
     - 過了 `cache_ttl_secs` 才整份追平。
   - 不為此丟線。補丁會留在該 owner 自己的歷史裡；換人時已經依第 4 點開新 session，`applied` 跟著重置，所以補丁不會跨角色續用。

**C. 狀態區塊加「以此為準」標記〔作者裁決 2026-10-07〕**
- 現況：
  - 狀態區塊由 `transport/state_view.rs` 的 `character_state_block` 組，只有 lane 回合尾段（`transport/turns.rs`）用到。
  - 每輪標題都是同一句「「X」目前的狀態（系統帳，唯讀；…）」，單角色線不抹後，歷史裡會有 N 份同標題區塊。
- 標題改成「以本區為準，取代先前對話中所有同名狀態區塊」之意的固定句，中英兩版。
- 對快取的影響：
  - 標記只出現在當輪最後一則 user，在快取項之外，不影響命中；
  - 既有線的歷史保留舊標題，前綴不變，不會額外重寫；
  - 凍結 system 不受影響。
- 連帶要改：`transport/state_view/tests.rs` 的標題斷言，以及重產 `lanes/scaffold_baseline/*.txt` 基線（只有 zh-TW／zh-CN／zh-HK 三份；英文骨架由其他測試核對）。
- **整塊消失時丟線重開**〔作者裁決 2026-10-07〕
  - 分支清空、解除綁定、節點不在或 body 為空時，`character_state_block` 回 None（`state_view.rs:351` 起），新標題撤銷不了歷史裡的舊值。
  - 做法：
    - `LaneState` 新增 `had_state_block: bool`（`#[serde(default)]`，舊檔為 false，不會誤丟）；
    - claude 角色線單角色模式呼叫前寫入本輪有沒有狀態區塊，其餘線一律 false；
    - `plan_turn` 依 B-4 的順序判定：`unerased_owner` 有值、上輪為 true、本輪沒有狀態區塊時，以 `StateBlockGone` 重開；這一項排在 Resume 之前，不會被「同一人續用」短路。
  - 不採撤銷訊息。理由：這種情況少見；撤銷句要新增十語系提示詞字面，而且只出現一次，之後仍和舊值並存；重開一次最乾淨。
  - 欄位部分刪除時，區塊仍在、新標題已寫「取代」，不丟線。
  - 不再觸發的限定條目留在歷史，視同角色記得的知識，不另處理。〔作者裁決 2026-10-07〕

**D. 偵測**
- 時機：每輪成功收尾後。
  - 有 rewrite（claude 角色線，單、多角色都會補前綴）時，共用 `apply_rewrite` 已載入的 `SessionFile`。
  - 沒有 rewrite（GM 線）時，另做只讀掃描：逐行解析 JSON，壞行略過，不經 `load`／`write_atomic`。
- 比對：頂層 `type == "attachment"` 且 `attachment.type == "total_tokens_reminder"`，精確比對；一般文字裡出現這個字串不算。
- 找到時在 usage log 寫一行診斷事件（`event: "cli-reminder-seen"`，reason 為 `seen` 或 `scan-failed`，沒有 model）；同一個 session id 在一次 app 執行內只記一次。
  - `usage/report.rs` 的迴圈在最前面就跳過這種診斷事件：不加 `worlds.rounds`、不計 `events`、不佔「最近一輪」燈號（前端 `EVENT_KEYS` 也就不必認它）。
  - 不採「記在該輪既有那筆」：用量列在抹寫之前就寫好了，事後回頭改要重寫帳本行。
- 失敗語意分兩種：
  - GM 線那條獨立的只讀掃描：讀取或解析失敗只記診斷，不丟線、不刪檔。
  - 角色線共用 `apply_rewrite`：偵測只讀已載入的內容、本身不會失敗；`apply_rewrite` 的載入、抹寫、前綴失敗照原本處理（丟線或留 pending），不會被偵測吞成診斷。
- 偵測不改 session 檔的任何位元組。

**其他**
- `lanes/mod.rs` 模組註解「命中率天花板＝只有最後一句沒中（E6：99.7%）」改寫成現況：每個 request 只有最後一個斷點可重用、抹寫會作廢它、開線後第 2 輪一次性損失。

## 四、驗收

**單元**
- `claude_cli_envs` 帶 `CLAUDE_CODE_TOTAL_TOKENS_REMINDER=off`。
- 模式判定：
  - 單卡判單角色；
  - 已回歸的 auto_hidden 卡算在場，未回歸的不算；
  - 開口者不在 cards 裡時仍納入；
  - 零卡判單角色；
  - 同名不同 id 時判多角色。
- 四種線的開關組合符合 B-2 的表；measure.rs 與 chat.rs 的組裝一致。
- lanes.json 相容：
  - 舊檔混合 claude、GM、agy、grok 各線，缺新欄位也整份讀得回；
  - claude 角色線讀成 `unerased_owner = None`、`had_state_block = false`，照舊每輪抹。
- `plan_turn`：
  - 前線 owner＝X、本輪 X 單角色 → Resume；
  - 前線 owner＝X，換人、別的角色共用同一 key → OwnerChanged；
  - 前線 owner＝X、本輪多角色 → ModeChanged；
  - 前線 owner 為 None＋多角色 → Resume，呼叫前寫入 None；
  - 前線 owner 為 None＋單角色 → Resume，呼叫前記下新 owner；
  - 前線 owner＝X、本輪同一人但狀態區塊從有到無 → StateBlockGone（不被同人續用短路）；
  - 判定順序照 B-4，pending 等既有條件優先；
  - agy 單角色線（一角一線）遇到狀態區塊消失、在場變多：照舊不重開，owner 與 `had_state_block` 恆為 None／false；
  - 私設改字且在 TTL 內 → Resume 帶補丁；
  - 私設在 TTL 外 → rebased Resume。
  - 私設要涵蓋新增、修改、清空三種，以及更新後換角。
- 偵測：
  - 認出 attachment；
  - 一般文字提及這個字串不誤報；
  - 同一個 session 不重複記；
  - GM 線掃描讀壞檔：只記診斷、不丟線；
  - 角色線 `apply_rewrite` 載入失敗：照原本丟線，不被偵測吞掉；
  - 掃描前後檔案位元組相同；
  - 報表：診斷事件行不加 `worlds.rounds`、不計 events、不改最近一輪燈號。
- 狀態區塊新標題；`npm run verify` 全綠。

**假 CLI 隔離測試**
- 假 CLI 記錄每次實際收到的 session id、是 `--session-id` 還是 `--resume`、stdin 與 system 檔內容，以及環境變數。
- 路徑：
  - 單角色成功、中止、runaway、resume 失敗後降級、開線失敗、前綴改寫失敗；
  - 留著 pending 重啟後換角；
  - 離場換人；
  - 同一人回來；
  - 在場變 2 但開口的仍是原角色；
  - 第二個角色解析到同一模型而改掛同一 key；
  - A 在 Sonnet 留下未抹線 → 換 Opus 進多角色 → 切回 Sonnet 時仍是多角色；
  - 狀態欄位部分刪除；
  - 整個分支清空、解除綁定（走真的狀態資料，不手填旗標）。
- 斷言：
  - **仍含未抹機密**的 session id（owner 有值的線，或留著 pending 的線），之後絕不再以 `--resume` 送給別的角色或多角色模式；抹乾淨的多角色共線換角續用是合法的；
  - 換成 B 時：新 session 的 system 與 stdin 不含 A 的私設、限定條目、狀態區塊；
  - A 單角色變成 A 多角色時：新 stdin 合法含 A 本輪的機密段（回合後再抹），只斷言沒有歷史殘留，也就是不 resume 舊 id、不帶舊輪的未抹內容；
  - 同一 owner 在中止、runaway、開線失敗、前綴失敗、留 pending 之後，下一輪仍必須是新的 `--session-id`；
  - Sonnet／Opus 反例：切回 Sonnet 時開新 id，舊 Sonnet 線不被 resume；
  - 欄位部分刪除：新狀態區塊是新標題，不含已刪欄位，舊 session 照常續用；
  - 分支清空、解除綁定：假 CLI 實際收到新的 `--session-id`。
- 另核對：
  - GM 線、卡重構線的 session 不和角色線混用；
  - 子程序實際收到 `CLAUDE_CODE_TOTAL_TOKENS_REMINDER=off`。

**測試通道（claude Sonnet、凍結場景）**
- 命中：GM 線與單角色線各跑 4 輪。
  - 第 3、4 輪的 read 應等於前一輪 read＋creation（完整命中）；
  - 第 2 輪允許只命中 system 段。
- 多角色桌：第二個角色開口時開新 session，之後每輪抹寫，私設不外洩。
- 機械閘門：新標題出現在當輪狀態區塊、狀態整塊消失會開新 session，以及第 3、4 輪完整命中。
- 狀態變化多輪是觀察項，不是閘門〔模型判斷·未裁決〕：
  - 用有狀態欄的單角色卡跑至少 5 輪，期間同一欄位至少變動 3 次，例如數值升降、物品得失；
  - 照實記錄每輪回覆引用的值；若看到模型引用舊值，回報主線，不擋結案；
  - 最後一輪的 read 照實記錄。

**實測結果（2026-10-07，測試通道，claude Sonnet，乾淨環境啟動；每格為 cache read／creation）**

單角色線（狐狸，狀態每輪改）：

| 輪 | read／creation | 說明 |
|---|---|---|
| 1 | 0／1,216 | 開線 |
| 2 | 546／1,469 | 只命中 system 段，即開線後第 2 輪的一次性損失 |
| 3 | 2,015／801 | 完整命中 |
| 4 | 2,816／893 | 完整命中 |
| 5 | 3,709／855 | 完整命中 |
| 6 | 546／3,795 | 清空狀態分支後以 state-block-gone 開新線 |

GM 線：

| 輪 | read／creation | 說明 |
|---|---|---|
| 1 | 0／5,210 | 開線 |
| 2 | 0／6,091 | GM 的 system 不到最低快取門檻，所以讀 0 |
| 3 | 6,091／985 | 完整命中 |
| 4 | 7,076／1,024 | 完整命中 |

多角色：
- 騎士開口時以 owner-changed 開新線，之後共線每輪抹。
- 騎士那條 session 裡，狐狸私設出現 0 次。
- 共線 session 只在 queue-operation／last-prompt 兩種行留有私設，對本機假端點 resume 時 request 裡為 0 次。

觀察項（狀態變化多輪）：HP 依序 100、80、55、30、65，背包變動 4 次；模型 5 輪都引用當輪的新值。
