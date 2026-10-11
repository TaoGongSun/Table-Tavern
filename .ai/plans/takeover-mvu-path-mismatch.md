# takeover-mvu-path-mismatch：MVU 卡重構後 GM 對不上變數路徑

立案見 [handoffs/takeover-mvu-path-mismatch.md](../handoffs/archive/takeover-mvu-path-mismatch.md)；實測數據見 [interface-takeover-card-types](interface-takeover-card-types.md)「發現的程式問題」第 2 項。方案經 Sol＋兩位 Opus 兩輪審查（根因 PASS），以下已併入兩輪必改與採納建議。

## 根因（零 AI 重放前次產物：scratchpad `rt/root` 的 DM 接管桌 `01M4ND45…` 與基準桌 `01M4ND2S…`，GM 實際提示詞取自 claude CLI session 檔）

| 症狀 | 真正原因 |
|---|---|
| `[mvu_update]变量输出格式`／`强调` 被淘汰 | 判官依 rule ①（輸出容器紀律）淘汰。內容是 `<UpdateVariable>` JSONPatch 寫法，App 的「狀態更新協定」已等價提供，**淘汰本身沒錯**〔模型判斷·未裁決〕。 |
| GM 看不到變數結構 | `[mvu_update]变量更新规则`（型別定義＋每欄何時怎麼變）被 absorb 成 locked 機制條目。`refactor/apply.rs` `new_entry_value` 對 locked 條目一律寫 `constant=false、keys=[]`——**永遠觸發不到，GM 從此看不到這條**（基準桌三條 mvu 條目都在回合尾段，GM 9 條路徑全對）。接管桌 GM 提示詞裡唯一的路徑線索是通用協定範例 `/Heroes/亞瑟/Affection`、`/World/Location`，於是自創 `/Dungeon/...`、`/DungeonMaster/...`。 |
| 規則欄位名轉繁體、狀態樹仍簡體 | absorb 提示詞要求「JSON 的 key 用玩家語言」、`MECHANISM_FIELD_SCHEMA` 說路徑「不含分支前綴」，而 refactor 脈絡**沒有附這桌既有的狀態樹**（MVU 卡匯入時已由 initvar 建好，簡體）。AI 只能照盤點 FIELDS（同樣被要求玩家語言）自取繁體扁平名（`迷宮之主位置`、`撤退進度`）。`mechanism::rule_for` 只認從樹根起算的完整點分路徑（可 `*`），36 條規則、6 條觸發幾乎全部對不到任何節點。 |
| 本地規則失效的連帶 | 規則對不上→文字葉子退回預設 `inject=snapshot`，對齊回合之後 GM 的「目前狀態」只剩 2 個數字欄，看不到樹的形狀。 |
| insert 長出孤兒枝 | GM 的錯路徑 insert 會自動補中間分支。屬上游症狀，路徑對了就不會發生；不改 insert 語意（無殼桌靠 insert 長樹）〔模型判斷·未裁決〕。 |
| 每回合輸入 16K→28K→47K | 不是本案缺陷：CLI 續聊線的「尾段世界書」設計（`lanes/plan.rs`，plans/tail-worldbook-erase.md）。三回合新建快取 16.4K／11.8K／19.1K，其餘是快取讀；未重構桌同樣如此。本案不處理〔模型判斷·未裁決〕，另案與否由主線決定。 |

## 改法

### 1. 有欄位規則的 locked 機制條目照來源觸發給 GM（`refactor/apply.rs` `new_entry_value`）
- 判準：**篩選前**的 `entry.rules` 非空＝這條在定義 GM 要回報的欄位，GM 必須讀得到「何時怎麼變」。只有觸發表、沒有欄位規則的 locked 條目（觸發劇本）維持現行隱藏（`constant=false、keys=[]`）。
- 寫法：**單一**來源核對得到時走 carry 同一條路——複製來源原始值（position／depth、次要鍵、selectiveLogic 等全帶），只換內文，強制 `locked=true`、可見度 `Gm`、`is_person=false`；核對不到退回 meta 建（落 position 0，屬預期）；無 meta 的多來源合組照走 `new_entry_shape`。照原卡位置：DM 那條在尾段（position 4，基準桌 9/9 路徑全對）；作者評估放前段吃快取的遵守度風險後決定照原卡放尾段〔作者裁決 2026-10-11〕。
- 篩選（改法 3）後規則與觸發都空的 locked 條目改成 `locked=false`、照來源觸發，但仍強制 `Gm`、`is_person=false`，避免內容整條對 GM 消失。
- 原地改寫 `refactor/tests/visibility.rs:375-385` 的舊斷言與 [worldbook-character-visibility](worldbook-character-visibility.md) 第 114、155 行「機制條目一律 GM、非常駐、無主鍵」的結論（原為主線裁決，本案推翻）。

### 2. 重構提示詞附上既有狀態樹
- 範圍：盤點（`survey.rs`）、absorb／合組（`rewrite.rs`）、介面展開（`expand.rs::expand_messages`）四種 user 訊息。狀態樹非空時多一段「既有狀態樹路徑」：葉子點分路徑逐行，空分支註明「子鍵由劇情新增」；超過 200 行就截斷，但**所有頂層鍵仍完整列出**，並標明「非完整清單」。樹取自 `data::read_state`（已含卡片變數投影）。
- 優先序（比照 `prompt_common.rs:86` 前例寫明）：附了既有狀態樹時，FIELDS、RULES 鍵、TRIGGERS `when.path`、STATE 鍵、SHELL 佔位符路徑照樹逐字寫、從樹根寫完整路徑，**不受玩家語言與繁簡規則約束**；值與說明照玩家語言。
- 萬用 `*` **只限 RULES 鍵**（動態鍵如房間編號、人名）；TRIGGERS `when.path`／`flag`、STATE 實際鍵、SHELL 佔位符一律具體絕對路徑；逐人／逐房間門檻不抽成觸發。
- TRIGGERS 的 `flag` 不照樹逐字：用新鍵名，放在既有頂層分支下或專用分支，不可指到既有資料葉子（once 命中會把它釘成 "true"）。
- 要改的衝突句：`rewrite.rs:46`「含 JSON 的 key 與值使用 {lang}」、`survey.rs:70`「欄位名一律玩家語言」、`prompt_common.rs:36` `known_fields_line` 空清單時「由你命名（玩家語言）」、`prompt_common.rs:51` `INTERFACE_STATE_RULES`「欄位名一律玩家語言」（`:86` 只讓卡原文詞彙優先，要加既有樹）、`prompt_common.rs:124-126` schema 結尾、system 前言「禁止繁簡並存」。system 前言要逐位元組不變才吃快取，不動它，在 user 段明寫本條優先。
- `MECHANISM_FIELD_SCHEMA` 的「不含分支前綴」無條件改成「從狀態樹根起算的完整點分路徑」（與 `rule_for` 實際比對一致）。

### 3. 套用時篩掉對不到樹的新規則與新觸發（`refactor/apply.rs`）
- 時機：在 entries 迴圈前算好（套用前的樹與介面 preflight 都已就緒），寫條目時就知道篩完是否全空。
- 要不要篩：看**套用前**的樹（`apply_into` 開頭 `read_state`）。套用前樹空就整批不篩。非空時比對集合＝套用前頂層鍵 ∪ 本次真的套用的介面（`normalized_interface` 有值）經 `normalize_interface_paths` 正規化後的 `state_fields` 頂層鍵。
- 篩選對象只有本次新增的候選（entries 與舊 `mechanisms` 兩處）；既有 `state.mechanism`（匯入規則、旗標 ReadOnly、手標計數器）不參與篩選，有效新候選仍照現行順序覆蓋同路徑，被篩掉的候選不覆蓋。
- 規則：首段不在比對集合就丟；首段是 `*` 的規則（合法萬用）不比對。
- 觸發：任一條件路徑（range／contains／flag）或 `trigger.flag` 有一段是 `*` 的整組丟（`resolve_path` 不支援萬用，帶 default 的 range 會每回合用 default 求值，once 命中會長出字面 `*` 分支）；這條是例外，套用前樹空也丟；其餘 range／contains 條件路徑首段不在集合就整組丟；flag 條件、`trigger.flag`、帶 `default` 的 range 不列入首段比對。
- 有規則或觸發被篩掉時 `log::warn` 帶筆數。

### 4. 本桌更新範例擴到增量且 DeltaOnly 的桌（`transport/context.rs`）
- 條件：`incremental`、有規則、`numeric_update == DeltaOnly`（重構過或非 MVU 來源）時，不論有無 guide 都附 `table_update_example`。未重構 MVU 桌走 Upstream，提示詞逐字不變。有 guide 的接管桌輸出順序不變（guide→範例→介面歸屬聲明）；沒規則的桌逐字不變。
- 挑選：跳過 `kind: list`、略過含 `*` 的路徑；delta 取數字類欄（number／pair／counter），replace 取文字欄；找不到可用的就略過那一行，不拿文字 placeholder 示範數字 replace。
- `context.rs:304-306` 註解同步改。

## 已知限制
- 同一條目帶欄位規則又帶觸發劇本（逐日排程類常見）會整條給 GM 看，與原卡在 ST 的行為一致；觸發命中時該段文字會重複注入一次。
- 規則要在樹裡新長一個頂層分支（重構時還不存在）會被改法 3 篩掉；GM 照樣能 insert，只是那支沒有本地規則。
- 觸發表只支援絕對路徑；逐人／逐房間的門檻（每個冒險者的恐懼值）抽不成觸發，靠改法 1 讓 GM 讀原文自己判。
- 玩過一段才重構時，GM 先前自創的垃圾分支也會被當成既有路徑列給 AI、也通過篩選。
- 介面展開重建狀態樹時，STATE 沒列到的 initvar 頂層鍵會被捨棄（現行行為，本案只附樹降低機率）。

## 測試（cargo）
- `new_entry_value`：locked＋rules、單一來源核對得到 → 複製來源原始值（position／depth、次要鍵）、只換內文、`Gm`、locked；核對不到 → meta（position 0）；多來源合組無 meta → `new_entry_shape`；locked＋只有 triggers → 維持隱藏；篩完全空 → 不鎖、照來源觸發、仍 `Gm`；disabled 來源。
- 篩選：套用前樹空不篩；非空時首段不存在的規則與觸發被丟、存在的保留；介面 `state_fields` 新頂層算存在，未勾介面、characters 模式不算，鏡像折疊後的頂層才算；錯候選不覆蓋既有規則、有效候選可覆蓋；首段 `*` 規則保留且 `*.HP` 命中實際葉子（`rule_for_path`）；條件路徑含 `*` 的觸發整組丟；once 觸發的 `flag`、flag 條件、帶 default 的 range 不被誤丟；被篩時有 warn。
- 提示詞：survey／absorb／group／expand 在樹非空時含路徑段與優先序句、`*` 只准 RULES 鍵（不要求 when.path／flag／STATE／SHELL 用萬用）、flag 用新鍵名；超過 200 行仍列全部頂層鍵並標非完整；樹空時除 schema 那句外與現行相同（原「逐字相同」測試改寫）。
- `gm_system_prompt`：DeltaOnly＋無 guide＋有規則 → 附範例、路徑是真實規則路徑、跳過 list 與 `*`；只有文字規則時沒有 delta 行、只有數字規則時沒有 replace 行；無規則 → 逐字不變；有 guide → 順序不變、guide 規則含 `*` 時範例不取它；Upstream＋匯入規則 → 逐字不變；`incremental=false` → 不附。
- 混合：介面＋absorb 同卡時 STATE／SHELL／RULES 與既有樹一致；rules＋triggers 混合條目、keyword 命中／未命中時 GM 實送提示含／不含該條。
- 既有 refactor／transport／mechanism 測試全綠，`npm run verify` 12 步綠。

## 驗證計畫
1. **零 AI**：測試通道新桌匯入 `TestCards/DongeonMaster-safe.png`，用前次 `refactor-outcome.json`（繁體規則）套用：36 條規則丟 35、只留 `魔物`（list）；6 條觸發全丟（條件首段都不在樹、都沒 default）；沒有本桌範例（剩下的規則是 list）；條目因篩選前有規則而照來源觸發、GM 提示詞含 `<Variable_Update_Rules>`；完整路徑的 `rule_for_path` 命中結果正確。
2. **真模型**（約 $1）：同卡重構（Sonnet），檢查 FIELDS／RULES 用簡體真路徑、規則留下、GM 提示詞有本桌範例；切 Haiku 玩 3 回合（同前次三句玩家輸入）。記：patch 送出／接受數，被拒分原因（路徑不存在 vs DeltaOnly 擋下）；預定簡體葉子的實際值；無非預期新增枝（含英文與繁體孤兒枝）；無固定格式重印、無未變欄位整包重報；卡片變數模式實查：套用時先 handover_to_tree，下一回合 ensure_active 才可能切回 events——跑到第 2 回合後讀 control mode，確認該模式下規則照樣生效；輸出與輸入 token。套用率 ≥80%（前次 0/24、基準桌 9/9）留作參考，不是唯一標準。
3. **回歸**：NorthHall-structure 接管桌零 AI 檢查 GM 提示詞與修改前相比只有範例那兩行可能因挑選規則（跳過 list、`*`）換欄位，其餘逐字相同。

## 驗證結果（2026-10-11，測試通道，claude CLI）
- verify 12 步綠（cargo test 1675 過）。
- 紀錄位置（scratchpad `mv/root/data/worlds/`）：零 AI 桌 `01M4NMDX3TVAJVF9Q1PPSY6PV7`；真模型桌 `01M4NMF4XAQGNK1HN4QYCNPPG3`——control mode 在 `card-vars/control.json`、逐回合 patch 在 `transcript/0.jsonl` 的 `raw`、拒收在 `mechanism-log.jsonl`、葉值在 `state.json` 與各事件 `state`、GM 實送提示詞在 `~/.claude/projects/*-scratchpad-mv-root-config-cli-workspace/<lanes.json 的 session_id>.jsonl`、用量在 `mv/root/harness-ai.log`。
- **零 AI**：前次產物套到新桌——36 條規則只留 `魔物`、6 條觸發全丟、無範例；`[mvu_update]变量更新规则` 落 position 4、常駐、locked、GM。
- **真模型**（Sonnet 重構 3 次 $0.49；Haiku 3 回合 $0.36）：盤點仍淘汰兩條輸出格式；absorb 產 42 條規則全是簡體完整路徑（動態鍵用 `*`）、0 條觸發，套用後 42 條全留；GM 提示詞有本桌範例（`/基础信息/具体时间`、`/基础信息/已覆灭小队数`），回合尾段帶規則原文。
  - patch：9/9、8/8、3/3 全收，零「路徑不存在」、零 DeltaOnly 拒收；無新頂層枝（含英文、繁體）；insert 的小隊／魔物結構照 schema（`冒险者小队.碎石之刃.详细信息.撤退进度` 等）；正文無狀態重印，未變欄位不重報。
  - 卡片變數模式：套用後 `tree`，第 1 回合後切回 `events`，第 2、3 回合規則照樣生效。
  - 輸入 21.4K／39.0K／65.8K（快取讀 0／21.4K／39.0K），輸出 2522／3041／6196；第 3 回合輸出多來自 insert 整支小隊（4 名成員）。輸入增長是尾段世界書設計，規則條目（約 7.6K 字）在尾段每回合重送；依作者裁決照原卡放尾段。
- **回歸**：既有接管桌（NorthHall、Transfur）範例的 replace 行會從清單欄換成文字欄（`人物列表`→`地點`、`主動能力`→`力量.數值`），delta 行不變；scaffold 基準中 guide 規則全是 `*` 路徑的範例整段不再出現（原範例寫 `/Heroes/*/Mood`，GM 寫不出 `*`）。
