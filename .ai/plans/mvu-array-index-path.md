# mvu-array-index-path：沒重構 MVU 卡的陣列索引路徑

## 背景
變數模式的 stat_data 投影成狀態樹時，陣列是一片 JSON 文字葉子（`convert.rs` `stat_to_tree`）；樹模式 Upstream 桌（世界書 MVU）也是。一批更新走 `apply_block_typed` → `apply_updates_typed`，Upstream 桌另有帶型別的 `TypedView`，整批做完成 `TypedBatch` 由 `merge_tree_change_typed` 落檔。原本路徑一進陣列就斷在葉子，整筆拒收。

## 範圍〔主線裁定，依作者裁決 酒館行為照做〕
- 本案：索引路徑照上游套用；不做任何 schema 檢查。zod 版 MVU 卡上游 schema 是 any（MagVarUpdate `schema.ts` 的 zod hack），`getSchemaForPath` 回 null，本來就不查；TestCards 四張帶 `[initvar]` 的卡都是 zod。
- 非 zod（`$meta`／`$__META_EXTENSIBLE__$` 舊式）卡的 schema 模擬（陣列與物件可擴充、必填鍵、`$arrayMeta` template）另立 `mvu-meta-schema`（主線立）；本案對這類卡同樣不檢查，列已知偏離（另立案補齊，非排除）。

## 上游行為（MagVarUpdate `438f9ffc` `update_variables.ts`，已用 gh 拉原檔核對；墊片實跑確認）
JSON Pointer 拆段交 lodash。zod 卡 `targetSchema` 恆為 null。

| op | 索引段 | 行為 |
|---|---|---|
| replace／delta `/A/i` | `lodash.has`：只認正規十進位（`01`、`-1`、`-` 不存在） | 不存在報錯。replace：舊值 VWD 只改其第 0 項、舊值數字＋新值字串 `Number()`。delta：數字加總 `toPrecision(12)`、VWD 加第 0 項、其餘報錯 |
| insert `/A/x` | 末段 `/^\d+$/` 經值解析成數字（`01`＝1）；`-` 推尾；其餘字串 | 陣列：splice，超長推尾；字串鍵在陣列上設非索引屬性（序列化消失＝靜默）。物件：直接設鍵（不轉型）。容器 `undefined` 或原始型別報錯；`null` 且上一層為真值→建物件。**容器在頂層時查父路徑 `lodash.get(資料, "")` 恆為 undefined 而擋下**（`/物品/-` 失敗） |
| remove `/A/i` | 末段 `/^\d+$/`（`01`＝1） | 容器陣列且 i < 長度→splice；否則照原路徑 `has`，物件鍵照原字面（`"01"`）刪 |
| move | — | `updateVariables` 沒有 move 分支，整筆不套用 |
| insert 值 | — | 套用後 `cleanUpMetadata` 剝 `$meta`、標記、`$arrayMeta` 元素 |

## 做法（已實作）
- **攔截**：`apply_one` 在 `readonly_violation` 之後、依 op 分派之前，view 有效時沿路徑在樹上走，第一個葉子出現在中途且 view 在那裡是陣列 → `apply/array.rs`。move 在 `apply_move` 的 `take_node` 之前檢查兩端。
- **只轉換被操作的元素**：陣列取 view 的 JSON（`TypedView::json_at`／`marks_under`），被 replace／delta 的元素才經 `Val`（先照非有限數標記還原），其餘元素保留原始 JSON（大整數不經 f64）。做完整個陣列寫回葉子，樹寫成功才 `TypedView::replace_json`（文件與標記一起提交、標成 set 寫入）。`TypedView::get` 改成也走陣列索引（正規十進位）。
- **各 op** 照上表；replace 重用 `upstream_set`；delta 加總用 `to_precision_12`、不夾範圍。`delta …/X/0` 且 X 是數字 VWD（X 可以是陣列欄本身，或巢在元素裡如 `/A/0/HP/0`）→ 交 `vwd_delta_value`，套 X 的規則（含萬用 `A.*.HP`）夾值、Clamped 記在 X（與 `delta /X` 一致），寫回外層陣列；`delta` 直接打到元素本身是 VWD（`/列表/0`）照上游加第 0 項、不夾。insert 的值先 `clean(…, None)` 剝元資料；容器 `null`（不是批內 NaN）建物件；上一層不存在、陣列索引不存在一律拒收（不補稀疏洞）；物件鍵數字段 `01`→`1`（超過 15 位只去前導 0，輕防）。
- **非有限數與 splice**〔主線裁定，依作者裁決 2026-10-04 非有限數轉 null〕：splice 只移動位置、不改寫元素值，set 產生的標記只位移被 splice 那一層容器的直接索引段、被刪元素的標記丟掉，整批結束轉 null。前端墊片同步（`shiftSets`）。
- **守門**：陣列欄規則與完整路徑規則都過 `guard_refusal`；清單規則照套用。
- **變動標記**記在陣列欄：`delta /X/0` 且 X 就是陣列欄本身時寫帶號增量，其餘（含巢狀 X）寫「更新」。
- **insert `/物件/-`**（Upstream 桌，一般路徑）：`insert_node` 回傳樹上實際落的鍵（目前沒被用掉的最小整數），視圖與變動標記照它寫，落檔不再出現 `-` 與數字鍵重複。拒收記錄的 path 是完整路徑（點分）。
- **Upstream 桌 insert 新鍵**：值剝元資料後照 stat_data 投影（陣列、null 成葉子），寫進 view，之後同批可走索引路徑。
- **樹模式 `[initvar]`**：行內陣列／物件帶元資料時，匯入就剝掉（`strip_meta_leaf`），GM 狀態、狀態欄、卡片介面一致〔主線裁定，依作者裁決 罕見邊際狀況輕防〕。

### 偏離上游〔主線裁定，依作者裁決 酒館行為照做〕（後端與墊片一致，記在 `card-mvu-shim.md` 上游核對段）
- 整列 replace 舊值 VWD＋新值陣列 → 整個換掉（上游巢狀成 `[[…], 說明]` 寫壞清單）。
- 頂層容器 insert 不查父路徑（上游擋下 GM 最常寫的 `/物品/-`，照做會變爛）。
- splice 位移非有限數標記（見上）。
- 陣列路徑的 move、insert 末段非索引：上游靜默不套用，本地拒收回饋（回饋優於靜默；墊片照上游不套用，結果一致）。

### 拒收句（取代 `hint.rs` 陣列三句）
- 索引不存在：「/物品/5 不存在：/物品 目前 3 項（索引 0–2）；改項目請 replace /物品 寫出整個陣列。」空陣列「目前是空的」；replace 索引恰等於長度改「新增請用 insert /物品/-，或 replace /物品 寫出整個陣列。」
- 元素物件鍵不存在：沿用「路徑不存在＋列現有鍵」（`hint::missing_group`，同前綴合併）。
- 撞到單一值：「/角色/1 是單一值，沒有子欄位；請 replace /角色/1。」
- delta 非數字元素：「…的值不是數字，不能 delta；請 replace … 寫新值。」；增量不是有限數：「…是數字，delta 的值要是純數字增減量（例 5、-10）。」
- insert 末段非索引：「/物品 是陣列，insert 的最後一段要寫索引或 -（例 /物品/-）。」
- move：「/物品/0 是陣列項目，不能 move；請 remove 後 insert，或 replace /物品 寫出整個陣列。」

## 測試
- `mvu-replace-parity.json` 新增 17 例（`schema_any: true`：墊片以 schema any 跑，測的是 MagVarUpdate 在 schema any 時的行為，不是 tavern_resource `mvu_zod` 自行套用指令的行為），Rust `replace_parity_tests` 與墊片兩端都過：各 op 的索引、`01`、越界／`-`／`-1`、頂層 insert、VWD 整換、剝元資料、null 容器建物件、既有鍵覆蓋、物件鍵 `01`、批內交錯、非有限數位移（刪前面、插頭、巢狀、刪含標記子樹、改回有限值與 null）、新鍵陣列。
- `apply/array_tests.rs`：VWD 第 0 項夾值與 Clamped（含巢狀 `/A/0/HP/0` 套萬用規則，萬用規則唯讀／骰值時拒收）、元素不夾、move 兩端拒收、insert 非索引、守門、各拒收句、跳脫與 240 字、DeltaOnly 不變、新鍵投影。
- `vwd_delta_tests.rs`：索引路徑持久化、兩字串清單、大整數原文保留、`insert /物品/-` 後同批 `replace /物品/0` 落檔無重複鍵。`import/mechanism.rs`：initvar 剝標記。
- 改既有：`hint_tests.rs` 陣列兩組、墊片 schema 規則那條（頂層放行）。

## 已知偏離與限制
- 本案的上游基準是 MagVarUpdate 路徑；zod 卡實際由 tavern_resource `util/mvu_zod.ts` 在 `mag_command_parsed_for_zod` 自行套用並清空指令（set 直接 `_.set`、add 無 `toPrecision`／VWD、insert 空容器先試 `{}` 再 `[]`、move 會搬、`_` 開頭靜默、每條過 zod `safeParse`），行為不同，交 `mvu-zod-semantics` 由作者定方向。
- 非有限數標記兩端不一致〔主線裁定，依作者裁決 罕見邊際狀況輕防；且 zod 卡基準待 mvu-zod-semantics 定案〕：
  - VWD 第 0 項 set 出 NaN 時，墊片標記指向整個 `[值, 說明]`（`vwd: true`），後端指向第 0 項。反例：`HP=[50,"x"]`，`replace /HP "很多"` 後 `insert /HP/0 9`——後端標記隨 splice 移到第 1 項、落檔 `[9,null,"x"]`；墊片檢查 `HP[0]`（9）不轉、NaN 留著而丟上限錯。
  - set 出 NaN 後同批 delta：`A=[1]`，`replace /A/0 "很多"` 再 `delta /A/0 5`——後端重記標記、落檔 `[null]`；墊片 `applyAdd` 撤銷標記、NaN 留著而丟上限錯。
  - 兩例都只讓墊片該次 parseMessage 拋錯，未見錯位落檔或改寫原資料（Sol 複驗），保留給 mvu-zod-semantics。
- `insert /物件/-`（物件不是陣列）：上游寫字面 `"-"` 鍵，本地寫最小未用整數鍵（既有本地行為），兩端不同，未進 parity。
- Upstream 桌 insert 到已存在的鍵：頂層／一般路徑走 `replace_existing`（照上游 set 轉型，VWD 只改第 0 項），陣列元素內照上游 `setOwn` 直接覆蓋、不轉型，兩者不一致（一般路徑那邊是既有本地差異）。
- 非 zod 卡的 schema 檢查未做，交 `mvu-meta-schema`。
- 中間層不存在時本地 insert 自動建分支（上游 undefined 報錯；既有本地差異）。
- Upstream 桌 insert 新鍵的 null 葉子由 `""` 改成 `"null"`、陣列由索引分支改成一行 JSON（與 stat_data 投影一致；樹模式非增量桌的狀態欄顯示隨之改變）。
- 一般路徑 move 照搬（上游整筆不套用），既有本地差異〔模型判斷·未裁決〕。
- VWD 第 0 項是日期時不做上游的加毫秒；元素是日期字串的 delta 同樣拒收。
- insert 值自帶標記讓批內新陣列變可擴充，屬 schema 範疇，隨 `mvu-meta-schema`。
