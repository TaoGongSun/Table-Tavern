# table-status-panel：通用狀態欄（方案）

案別：大案，核心估約 720（Rust 約 370、桌面前端約 220、房間協定與手機約 130）；方案與驗收各送 Sol 一輪起（上限 2 輪）。分工：包 1 opus-medium 子代理施工、Sol 驗收；包 2、包 3 改 Sol 施工、opus-review 驗收（Sol 額度近日重置兩次，本週 Grok 用量不足）〔作者裁決 2026-11-07〕；每包審查員與實作者供應商不同。全篇取捨未標來源者為〔模型判斷·未裁決〕。

## 0 開工重讀現況（範圍重估的依據）

- **三鍵早就每桌都在收**：`transport::response::narrate_instruction` 不分桌一律要求 ```state 圍欄（time／place／present），值存 `state.table`，逐則快照、收回回滾、手改、換幕對齊全部已有。單人桌「沒有狀態欄」只是前端 `AppWorkspace.tsx:339` 的掛載條件 `hasStateBar || tree 非空`；房間端三鍵更已經在每個人手機上（room-gm-status-block：`gen.part.end.state`、`seed.rs:258`、`sync.rs:140`、`SceneStatusLine`）。本案不必再做「三鍵」，只補三鍵以外的內容。
- **兩份 GM 迴圈只差落檔**：`room/run.rs` 透過 trait 呼叫的仍是後端 `chat_turn::gm_narrate`，狀態解析、套用、提交（`gm_commit_apply`）、`status` 三鍵（`gm.rs:369`）都在後端一份；單人前端 `useChatController` 收完 `refreshState`，房間端 `land_narration` 落檔後廣播。所以種入、規則、預算、提示詞全部只寫後端一次，兩條路都吃到；gm-relay-unify 不在本案範圍。
- **只報變動的機制全在**：`Mechanism.incremental` 開啟後系統提示附統一協定（`context.rs::delta_only_protocol`）＋本桌範例（`table_update_example`）；`<UpdateVariable>` JSONPatch 五種 op 解析與依規則套用（`mechanism::apply`）；`FieldRule{kind,update,inject,allowed}`、`locked_roots`、`write_guard`、`state_scope` 依在場裁分支、角色線自帶自己分支（`character_state_block`）。
- 一期交接檔寫的 `TableState.characters` 佔位已被二期的 `tree` 取代，本案的角色格直接是樹的一支。
- 要繞開的四個坑（實作前已查證）：
  1. `mechanism.guide` 非空會連帶附 `interface_owned_notice`（「不要輸出圍欄、全部寫進更新區塊」），與本案保留 ```state 圍欄相衝，**不得借用 guide**。
  2. `write_guard::write_refusal`：`locked_roots` 底下對清單 insert／remove 走 `Write::Other` 會被拒，**不能用 locked_roots 保護預設樹**。
  3. `table_update_example` 跳過含 `*` 的路徑與清單欄，只用萬用規則模型拿不到具體範例（Haiku 會照抄通用範例的 /Heroes）。
  4. 模型寫樹有兩個入口：`<UpdateVariable>` 補丁與 ```state 圍欄內的巢狀欄位（`block_paths::align`→`guard_fence`），預算要兩邊都夾。

## 1 玩家看到什麼

- 空桌、一句話開桌、房間一句話開世界：第一次 GM 旁白落段後，對話頂端出現一行「時間 · 地點 · 在場 N 人」，點開是面板：每個角色一格（一句話狀態、持有物短清單）、「追蹤中」三欄（目標／承諾／線索）。桌面版點開是對話右側一欄，手機（房間玩家）點開是底部抽屜；兩者都不蓋住對話。〔作者裁決 2026-11-07：欄位三組、只報變動、硬預算、一行摘要、不蓋對話〕
- 每格可點改（桌面版）；超額的字自動截掉；模型亂寫不會中斷遊戲。
- 有卡格式的桌照舊顯示卡自己的欄位，但容器同一個（同一列、同一個側欄）。

## 2 資料：預設樹與規則（後端）

### 2.1 哪些桌、何時種

- 條件（全部成立）：`table_kind == Chat`、非 `replay`、非介面接管（`is_interface_takeover`）、`world_has_state_bar` 為假、`mechanism.rules` 與 `state.tree` 都是空的（沒匯入過任何機制）。
- 時機：`gm_narrate` 取 materials 前（已持 world write permit）呼叫 `data::default_panel::ensure(root, world_id)`；冪等：第一次種整棵＋規則＋旗標，之後每回合只補缺的角色格、搬改名的格、刪掉已不在桌上的格。**沒有變動就不寫檔**（規則進系統提示，寫了快取就滅）。
- 旗標：`Mechanism.default_panel: bool`（新欄位、落檔）。不借 `incremental_by_refactor`（它改 `leaf_shown` 語意）。種入時 `incremental = true`。
- **不被當成 MVU 桌**（Sol 第 1 輪必改 3）：`mechanism/policy.rs` 的 `resolve_numeric_update` 與 `mvu_source` 兩處都以 `default_panel` 排除——預設桌一律 `DeltaOnly`、不是 MVU 來源；這樣 `context.rs` 無 guide 的 DeltaOnly 分支才會附本桌範例，補丁也不走 `upstream_set` 的型別寫回。驗收從 `gm_materials` 載入一路測到實送提示詞含固定範例，不只測範例函式。
- 種過就是預設桌：之後匯入帶格式的卡不切換（玩到一半換格式會把資料弄亂，列已知限制）。

### 2.2 樹形（鍵固定英文，顯示才在地化）

```json
{
  "characters": { "<卡名>": { "status": "", "items": [] } },
  "tracking":   { "goals": [], "promises": [], "clues": [] }
}
```

- 鍵用固定識別字，顯示名走既有的 time／place／present 作法：`state_view` 的顯示名對照表擴充（時間／地點／在場人物旁加 角色／狀態／持有／追蹤／目標／承諾／線索，zh／en 兩版，與腳手架語言一致），前端 StateBar 用 i18n 鍵。模型回報的路徑不隨介面語言變。
- 角色格：玩家卡（含房間多張玩家卡）＋桌上非封存、非舞台（`stage`）卡，各一格，**格鍵＝卡名（模型要讀得懂路徑），但歸屬以卡 ID 記**：`Mechanism.panel_cells: BTreeMap<card_id, 格鍵>`（落檔）。ensure 每回合對照：ID 在、名字變了 → 把分支整支搬到新鍵、更新對應，資料與自訂欄都留著（Sol 第 1 輪必改 1）；ID 不在桌上了 → 刪格；同名兩張卡 → 第二張的格鍵加「（2）」後綴。隱藏／離席的卡保留格（GM 看不到：`state_scope` 的手足裁切對 `characters` 容器成立，因容器不是樹根）。角色線（chars lane）靠 `auto_match_branch` 自動綁到自己那格。人物條目（世界書）不給格。上限 12 格，超過的卡不給格。
- 在場（present）照舊在平欄，不進 characters；時間地點也不重複。

### 2.3 規則與預算

| 路徑 | kind | update | inject | 預算 |
|---|---|---|---|---|
| `characters.*.status` | text | replace | turn | ≤ 60 字 |
| `characters.*.items` | list | replace（整串） | turn | ≤ 8 項，每項 ≤ 24 字 |
| `tracking.goals`／`promises`／`clues` | list | replace（整串） | turn | ≤ 6 項，每項 ≤ 48 字 |

- `FieldRule` 加兩個選填欄：`max_chars: Option<u32>`（葉子值字數，`chars().count()`）、`max_items: Option<u32>`（清單項數）。預算是資料，不是提示詞；協定裡只用一句話告知上限（靜態、可快取）。
- **清單只收父路徑整串陣列**（Sol 第 2 輪必改 1）：預設桌是 DeltaOnly，`TypedView::for_table` 不啟用、`array_field` 回 None，`apply/array.rs` 的 insert／remove 路徑根本不會走到，所以不沿用它。預設桌的清單欄只收 `replace` 在父路徑（`tracking.clues`、`characters.<名>.items`）且值是**字串陣列**；逐項補丁（`…/clues/0`、`…/clues/-`、insert／remove）一律拒收並回饋「這欄請用 replace 寫整串陣列」。`list_protocol::push_list_protocols` 的「多行字串整串 replace」指引在預設桌**略過**（它講的是字串形狀，與陣列相衝），改由 2.4 的指引與固定範例講陣列寫法。
- **清單項目的注入繼承只限預設桌**（Sol 第 2 輪必改 2）：`rule_for`／`leaf_shown`／`has_rule` 加一條只在 `mechanism.default_panel` 生效的規則——路徑恰為「有 list 規則的父路徑＋一段純數字索引」時，繼承父規則（inject turn，非對齊回合 GM 才看得到項目）；其他深度、其他鍵名不繼承，既有卡格式桌的語意一字不變。寫入資格不靠繼承：寫入只認父路徑（上一條）。
- 夾法（新模組 `mechanism/budget.rs`，一個函式兩個入口共用）：對清單先驗形狀——父路徑收到的值必須是字串陣列，物件、巢狀、非字串項目整筆丟掉不收（記帳、回饋一句）；再夾整份清單：超項只留前 N 項、單項超長截前 N 字；文字葉子超長截前 N 字。截斷**靜默**，記一筆 `RecordKind::Trimmed` 進 `mechanism-log.jsonl`，不產回饋句、不報錯。兩個入口：`apply_one`（補丁）與 `block_paths::align`／`guard_fence`（圍欄巢狀欄位）。
- 表外寫入守門（`write_guard` 加一段，只在 `default_panel` 桌）：`characters.*` 與 `tracking.*` 底下只收「有規則」的路徑（清單＝父路徑）；新增表外鍵、刪除／搬移整格、整支替換 `characters`／`tracking` 一律拒收並回饋一句（沿用既有回饋格式）。模型不能自己開格；開格是 app 的事（2.1）。
- 狀態＝「現在的快照」：status 每次 replace 整句取代，不是日誌；協定一句話講清楚。

### 2.4 提示詞

- `incremental = true` 已帶統一協定與拒收回饋。另加一段靜態「預設狀態欄指引」（新函式，zh／en，進凍結快照、措辭定稿後不改），只在 `default_panel` 桌附：三組欄位是什麼、status 是一句話現況不是日誌、持有物只列重要的、清單欄一律用 replace 寫整串字串陣列（加一項也重寫整串、解決的事就不寫進去）、各欄上限、沒變就不寫。
- `table_update_example` 在 `default_panel` 桌改出固定範例：`replace /characters/<第一格卡名>/status`、`replace /tracking/clues` 整串陣列（與 list_protocol 一致；卡名取排序第一格，格變動才變，與 2.1 的「沒變不寫」一致）。
- ```state 圍欄三鍵的要求照舊不動（`state_fence_rule` 一字不改）。

### 2.5 面板手動加欄位

- 新指令 `add_panel_field(world_id, parent_path, name)`：一次原子寫入——登記精確規則（text／replace／turn／max_chars 60）＋種空字串葉子（直接寫樹，不走 `set_state_path`：它先過 `write_refusal`、且空值＝刪除，Sol 第 1 輪必改 4）；之後改值走既有 `set_state_path`（有規則就過守門）。`remove_panel_field` 反向：刪規則與葉子。ensure 不動自訂欄（只搬／刪整格）。
- 桌面版面板：每個角色格與「追蹤中」各有「＋欄位」，輸入欄名即呼叫上述指令；欄名重複或是保留鍵（status／items／goals／promises／clues）拒絕。預設不多給任何欄位。〔作者裁決 2026-11-07：可手動加、預設不多給〕
- 數字欄（HP 等）不在本案：交 ttrpg-combat-tracker。

## 3 桌面版面板（前端 `src/features/table-state/`）

- 掛載條件維持 `hasStateBar || tree 非空`，預設樹種下即自動出現，不另加旗標查詢。
- 收合列（既有 `StateBar` summary）：單人桌照舊「時間｜地點｜在場」；房間模式照舊只畫入口（故事頁另有三鍵行）。
- **展開改成側欄**：對話區右側一欄（寬約 300px、自帶捲軸），不再把對話往下推；視窗寬度不足（約 < 960px）退回現在的「列下方展開」。卡格式桌同一容器同一行為（兩者共用容器〔作者裁決〕）。
- `default_panel` 桌的渲染：`characters` 畫成格（卡名標題、status 一行可點改、items 以 chip 列出可點改／刪），`tracking` 三欄清單；其餘樹（卡格式桌）仍走既有樹狀折疊。判斷依據：`WorldState.mechanism.default_panel`（`read_state` 已回整個 mechanism）。
- 刷新時機沿用控制器（切桌／旁白完成／收回／復原／`room-table-changed`）。

## 4 房間（協定＋手機抽屜＋桌面房主端）

- 房主端為權威：樹在房主桌上，玩家端只讀。〔作者裁決 2026-11-07〕
- 協定：`gen.part.end` 既有 `state` 三鍵不動，**加選填 `panel`** 欄（不得開新執行端訊框：未知訊框會 4400 斷線，見 process-log 2026-11-06）：
  ```json
  { "characters": { "<卡名>": { "status": "…", "items": ["…"] } }, "tracking": { "goals": [], "promises": [], "clues": [] } }
  ```
  只在 `default_panel` 桌送，整份快照（不是差異）；`seed.rs:258` 與 `sync.rs:140` 蓋最後一段時同樣附上。
- 伺服器 `validate.ts` 加 `cleanScenePanel`，與 `cleanSceneState` 並列：非物件整份丟；characters ≤ 12 格、鍵 ≤ 64 字、status 字串 ≤ 60 字、items ≤ 8 × 24 字、tracking 三鍵各 ≤ 6 × 48 字；型別不對的鍵丟掉；全丟就不附。上限數字與 2.3 同一組（伺服器端另抄一份常數，兩邊測試各自驗）。
- 客戶端合併：`panel` 整份取代上一筆（快照語意），沒有 `panel` 的段落沿用上一筆（與三鍵合併同一個函式家族 `scene-status.tsx`）。
- 手機：`SceneStatusLine` 的點擊從「展開三行」改成開底部抽屜（照 `card-sheet` 的做法）：頂端三鍵三行、下面角色格與追蹤三欄，唯讀；點外面關。沒有 `panel` 的桌（卡格式桌、舊段落）抽屜只有三鍵。桌面嵌入版（`DesktopRoom` 用 `RoomBody`）同一份元件。
- 桌面房主端：既有 StateBar 在房間模式照 §3 顯示（`hideSceneSummary` 不動）。
- 可見性（角色本人才看得到自己格的某些欄）本案不做，預設樹沒有私有欄；掛點（`characters.<卡名>` 分支、玩家卡綁定）留著給 v2。

## 5 分包與驗收（各包獨立 commit、各送一次驗收）

- **包 1 後端**（Rust，核心約 370）：`data/default_panel.rs` 種入、補格、改名搬格、刪格（`panel_cells`）、`FieldRule` 兩欄、`rules.rs` 預設桌清單索引子葉繼承、`mechanism/budget.rs` 兩入口驗形狀＋夾值、`write_guard` 表外守門、`policy.rs` 排除 MVU 推定、顯示名對照、指引文字、固定範例、`add_panel_field`／`remove_panel_field`、`Mechanism.default_panel`。測試：種入冪等不寫檔、補格刪格、**已有狀態後改名再旁白資料保留**、補丁與圍欄各超額截斷、逐項補丁拒收、整串超項截斷、物件冒充字串整筆丟、既有卡格式桌 rule_for 結果不變、新增項目後下一輪非對齊回合 GM 仍看得到、表外鍵拒收、載入後策略為 DeltaOnly 且實送提示含固定範例、角色線拿到自己格、裁切對隱藏卡成立、`cargo test` 綠。
- **包 2 桌面面板**（TS／CSS，核心約 220）：側欄版面、格與追蹤欄渲染、點改、chip 刪、＋欄位；i18n 十語（不計核心）。測試：掛載、格渲染、改值呼叫 `set_state_path`；測試通道截圖：收合列、側欄展開、窄視窗退回、卡格式桌同容器。
- **包 3 房間**（TS，核心約 130）：`panel` 欄位伺服器驗證、執行端附帶（Rust 約 20 行）、客戶端合併、手機抽屜、桌面嵌入。測試：驗證器邊界、合併快照語意、抽屜開關；手機 375×812 截圖：一行、抽屜、無 panel 的桌。
- 收尾：MECHANISM-FORMAT.md 加「預設狀態欄」節（樹形、規則、預算、守門）；docs/STRUCTURE.md:45「靠內容偵測自動掛上」改寫成「狀態欄：有卡格式照卡、沒有就種預設三組」；交接檔改寫；process-log 記一條。

## 6 已知限制〔模型判斷·未裁決〕

- 種過預設樹的桌，之後匯入帶狀態欄格式的卡仍是預設桌。
- 人物條目（非卡）沒有角色格；超過 12 張卡的桌只給前 12 張（照卡清單順序）。
- 第一次 GM 旁白前沒有狀態列（沒東西可顯示）。
- 房間端沒有 `panel` 的段落（開房當下沒有公開段落）要等下一則 GM 落段才有面板，與三鍵同一限制。
- 截斷靜默，玩家只在面板看到被截的值；mechanism-log 有記錄。

## 5a 施工結果
- 核心：Rust 約 1326 行（含提示詞文字與註解；`default_panel.rs` 407、`budget.rs` 206、`write_guard.rs` +168、`panel_prompt.rs` 86）、桌面前端約 422（含 CSS 113）、房間約 281（含 CSS）；測試 Rust 484＋前端 261＋party 254；i18n 十語不計。合計核心約 2000，立案估 720——Rust 的差距來自兩個寫入口各自要驗形狀、改名搬格連自訂欄規則、`gapped` 路徑對齊等審查補出來的信任邊界細節。
- 主線看截圖後小修：側欄追蹤三欄改直排、手機狀態列加箭頭。

## 6a 審查紀錄

- Sol 方案審 2 輪：第 1 輪必改 4（卡名鍵改名清格、清單規則段數、incremental 被判 MVU、＋欄位走不了 set_state_path）、第 2 輪必改 2（DeltaOnly 下 array.rs 不啟用、祖先繼承不可全域），全部採納；工程審到頂，方案定案。
- Sol 驗收包 1 兩輪：第 1 輪必改 3（畸形圍欄清單覆寫舊值、改名沒搬自訂欄規則、手改繞過預算）、第 2 輪 1（`gapped` 路徑沒做簡繁字形對齊；兩輪到頂由主線裁定修）；opus-review 驗收包 2 一輪（必改 4：remove_panel_field 參數形狀、清空自訂欄成幽靈欄、契約重複、status 單行截斷）、包 3 一輪（必改 2：按鈕預設樣式、抽屜位置）；全修。

## 7 觀察（範圍外，不立案）

- 卡格式桌在大廳進桌後才匯入世界書，`hasStateBar` 要重進桌才更新（App.tsx 註解既有行為）。
- 房間 `remote_control_replies_first_then_acts` 全套並行偶發紅、單跑綠，既有計時問題。
