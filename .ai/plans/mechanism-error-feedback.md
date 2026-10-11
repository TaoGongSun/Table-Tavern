# mechanism-error-feedback：GM 可修正的機制錯誤也回饋

## 現況
- `build_notes`（`mechanism/tree.rs:6`）只收 `Rejected`，最多 5 條、同文去重；由 `state_view.rs:93` 印成「上一輪被系統擋下的更新」。`Error` 只進記帳檔與面板計數。
- notes 只在**有套用的回合**整份覆寫（`ledger.rs:216`）：`chat.rs:402` 全量桌沒有狀態區塊就不套用，上一輪的 notes 原樣留著。
- 所有桌型（非接管桌、接管桌 `TakeoverReply` 的 `<UpdateVariable>`、沒重構的 MVU 卡 Upstream、全量／增量桌）都走 `apply_updates_typed`。接管桌開場白的 updates 在 `scoped_block` 先清掉；**非接管桌開場白也走 `apply_block_typed`**，錯誤會在第 1 回合回饋。
- 上游 MVU 對不存在的路徑 set／add／delete 也是報錯不建立（`src/features/card-interface/mvu/card-mvu-parse-source.ts` `applySet`／`applyAdd`／`applyDelete`），本案不改「不建立」，只補回饋。
- 解析層（`parse.rs:128` `value_to_patch`）把認不出的 op、缺 path、壞 JSON 物件整筆丟掉，連記帳都沒有；上游收 `"op":"add"`（＝insert）、move 目的地取 `path ?? to`（`card-mvu-parse-source.ts:326-360`），本地都不收。
- 沒重構 MVU 卡的 stat_data 陣列（含 `[值, 說明]`，下稱 VWD）在狀態樹是一片 JSON 文字葉子；`delta` 打到 VWD 上游加在第 0 項（`applyAdd`），本地推定成文字欄拒收。

## 主線裁定〔主線裁定，依作者裁決 酒館行為照做〕
1. 解析層併入本案：照上游收 `add`＝insert、move 目的地 `path ?? to`；其餘解析失敗改成回饋（見「解析層」）。
2. VWD 的 delta 本案補上；陣列索引路徑（`/HP/0`、`/清單/2`）另立案（主線立），本案對索引路徑只回饋。

## 分類表（`apply.rs` 行號以 main `2bd33f7` 為準）

| # | 情況 | 位置 | 現況 | 改成 | 理由 |
|---|---|---|---|---|---|
| 1 | delta 路徑不存在 | `:122` | Error | Rejected＋路徑提示 | GM 寫錯路徑 |
| 2 | delta 打到分支 | `:130` | Error | Rejected＋列子欄位 | GM 該對子欄位下 delta |
| 3 | delta：Pair 欄現值不是「現值/上限」 | `:155` | Error | 維持 Error | 帳本或規則壞掉，GM 改不了；發生機率低、記帳可查即可 |
| 4 | delta：數字欄現值不是數字 | `:185` | Error | 維持 Error | 同 #3 |
| 5 | replace 路徑不存在（含 `replace /神經狀態/0`、MVU 卡自創路徑） | `:261` | Error | Rejected＋路徑提示 | GM 寫錯路徑 |
| 6 | replace：Pair 規則但節點是分支 | `:341` | Error | Rejected＋列子欄位 | GM 該改子欄位 |
| 7 | replace：Pair 欄現值格式壞 | `:349` | Error | 維持 Error | 同 #3 |
| 8 | replace：Delta 規則但節點是分支 | `:384` | Rejected，訊息現值空白 | 分支時訊息改成同 #6；葉子時現值照印不變 | 現行訊息誤導 |
| 9 | insert 中間層撞到非清單葉子 | `:437` | Error | Rejected＋路徑提示 | GM 在單一欄位底下建子欄位 |
| 10 | remove 路徑不存在 | `:467` | Error | Rejected＋路徑提示 | 原本要刪的可能叫別的名字、還在 |
| 11 | move 來源不存在 | `:488` | Error | Rejected＋路徑提示（對 from） | 同 #10 |
| 12 | move 目標中間層撞葉子 | `:501` | Error | Rejected＋路徑提示（對 to）；原值放回 | 同 #9 |
| 13 | 衍生公式算不出來 | `derive.rs:81` | Error | 維持 Error | 規則／資料問題，GM 看不到公式 |
| 14 | delta 打到 VWD（Upstream 桌） | `:138` 前 | Rejected（推定文字欄） | 照上游套用（見「VWD delta」） | 上游合法 |
| — | upstream／Replace 分支內 `insert_node` 失敗 | `:310`、`:320` | 無記錄 | 不動 | 路徑已確認存在，不會失敗 |

`UpdateMode::Replace` 對分支整棵替換（`:318`）照舊允許；新 helper 只改原本就拒收或報錯的分支情況。

## 路徑提示 `path_hint`（`apply/hint.rs`）
簽名帶 `tree`、`mechanism`、`&TypedView`、要寫的路徑與 op；`apply_delta`／`apply_remove`／`apply_move` 改接 `view`（必要處接 `mechanism`）。提示與所有拒收句（含既有的骰值、唯讀、現值句與 VWD 拒收）的路徑一律寫成 GM 實際要寫的 JSON Pointer `/a/b/c`，每段 `~`→`~0`、`/`→`~1`，照抄回來 `parse_pointer` 拆得回原鍵〔主線裁定〕。第 4、5 類查規則用 GM 原本要寫的完整路徑（含萬用規則）。沿路徑往下走，依撞到的情況回一句；**每一類都先過該欄規則守門**：規則是 Local／Roll 回骰值句、Reject／ReadOnly／Derived 回唯讀句（沿用 `delta_refusal` 的句子），守門沒擋才給寫法。

1. **中途撞到 Upstream 桌的陣列葉子**（用 `view.get` 判斷，VWD 判定與 `upstream_set` 共用同一個 `is_vwd`；與清單規則同時成立時本類優先）：
   - 數字 VWD：「/HP 是 [值, 說明] 欄，不能用索引路徑；replace 它只會改第 0 項（值只寫新數字），增減用 delta 它。」
   - 第 0 項不是數字的 VWD（含恰好兩個字串的清單 `["劍","盾"]`，上游同樣當 VWD）：「/背包 依上游規則視為 [值, 說明]：replace 它只會改第 0 項（目前「劍」）；增刪項目目前不支援。」不寫「只寫新值」、不提 delta。
   - 兩種都**不得出現「整個陣列」**：`upstream_set` 對 VWD 只改第 0 項（`upstream_set.rs:94-123`），照「寫整個陣列」重試，數字 VWD 會變 `[null, "說明"]`（陣列轉數字是 NaN，序列化成 null），字串 VWD 會巢狀成 `[[…], "說明"]`。
   - 一般陣列：「/清單 是陣列，目前不支援逐項路徑；請 replace 它，值寫成整個 JSON 陣列（不要加引號）。」
2. **中途撞到清單規則的葉子**（沿用 `list_leaf_prefix`）：報清單欄本身，用 takeover-text-field-delta 的清單句（insert「不能逐行插入」，其餘 op「不能逐項改」）。
3. **中途撞到其他葉子**：「/X 是單一欄位，沒有子欄位；」後接依規則的寫法：Number／Counter／Pair「增減請 delta 它給增減量」；Text「請 replace 它寫新值」。（骰值／唯讀已由守門回對應句，不說「請直接更新」。）
4. **某一段不存在**：「路徑不存在：{第一個不存在的前綴}。上一層（樹根寫「頂層」）現有：k1、k2、…」——只印前綴，不印完整路徑，句中只出現這一個路徑。op 是 replace／delta、只缺最後一段時，補半句「若要新增請用 insert」；中間層缺或其他 op 不提。
5. **分支**（#2、#6、#8）：「/X 是分支，不能{做增減／整個替換成單一值}；請改底下的欄位：k1、k2、…」

### 列鍵規則
- 只列直接子鍵、略過 `_` 開頭，照樹的順序。
- **增量桌與渲染同一套**：略過 `InjectLevel::Rare` 的葉子、略過依同規則過濾後整支沒有可顯示葉子的分支（`state_view.rs:148-157`）；判斷抽成共用函式，`render_state_tree` 與列鍵同用。Snapshot 葉子照列（GM 換幕對齊時看得到）。全量桌照舊全列。
- 不在場角色分支（`scope.hidden`）照列——套用層拿不到本輪 scope，列已知限制。
- 不做相似名稱比對〔模型判斷·未裁決〕。

### 長度
- 一條 note 硬上限 **240 個 Unicode 字元**（`chars().count()`），以**整條 note** 計，所有拒收句（含 `build_notes` 直接沿用的單句）都過這道上限；路徑一律中段省略到 120 字內，指示句不截：所有路徑引用、指示、「本輪 N 筆未套用」、「等 N 項」省略尾綴都算在內。
- 組句順序：先排固定部分（路徑＋指示＋筆數尾綴），剩下的額度給列鍵，從尾端截並補「等 N 項」。固定部分本身超過上限時，路徑中段以「…」省略到放得下（句中第二次以後提到同一路徑一律寫「它」，不重複）。單一鍵名超過 24 字截斷加「…」。
- 列鍵上限：「某段不存在」20 個、分支提示 8 個，再受字元上限裁。
- 沿用 `build_notes` 5 條上限。

### 去重與順序
- 依來源順序逐筆處理：區塊依出現順序、區塊內依物件順序，解析拒收與套用記錄在同一條序列裡照原位置排，區塊層級 note 排在該區塊的位置（壞物件那則在第一個壞物件處，未閉合尾段與「沒有可解析」在區塊尾）。5 條上限下 GM 看到的是最前面的錯。套用後才產生的記錄（跳動、一次性旗標、衍生公式）排在後面。
- 「路徑不存在」去重：`Record` 加一個只在記憶體的分組資料（`#[serde(skip)]`）。只缺最後一段：鍵＝父層＋op 類別（replace／delta 一類、其餘一類），合併時開頭改「路徑不存在：{父層} 底下沒有這些欄位。現有：…」；中間層缺：鍵＝第一個不存在的前綴。`build_notes` 依鍵合併成一條，尾綴「（本輪 N 筆未套用：弓、藥）」列各筆末段鍵名（鍵名去重、筆數照實），整條受 240 字上限、鍵名從尾端截；帳本照舊一筆一筆記。其他類照既有同文去重。
- `apply_block_typed` 在全部記錄收齊後一律重建 notes，配合來源順序合併排序（現行 `ledger.rs:212-215` 只在有 scope_record 時重建）。

## VWD delta（Upstream 桌）
`apply_delta` 判定順序：`node_at` → 確認是葉子 → view 有效且 `view.get(path)` 是 VWD → **先套明確規則的守門**（Local／Roll 骰值句、Reject／ReadOnly／Derived 唯讀句，沿用 `delta_refusal`），只跳過「推定 Text／List」那段 → 第 0 項是數字才加總，照 `replace_existing` 的 upstream 寫法（`view.get` → 算值 → `insert_node` ＋ `view.set`）：第 0 項加 delta（上游 `toPrecision(12)`），說明不動。變動標記同一般 delta。
- delta 不是數字 → Rejected 數字欄句。delta 是數字字串（`"5"`）照現行 `value_as_f64` 收下——比上游寬鬆的例外，對 GM 有利〔主線裁定〕；但非有限輸入（`"NaN"`、`"inf"`、`"-inf"`）一律 Rejected、值不動（所有 delta 都適用）。有限輸入加總溢位成非有限數另算：VWD 照上游存（序列化成 null），一般數字欄維持現行。
- 有明確 min／max 規則就夾值，與現行 Upstream 桌一般數字欄 delta 一致——相容性例外〔主線裁定·理由：卡片自己定了範圍時夾值比上游好且與既有 Upstream 數字欄一致〕；沒規則不夾。
- 第 0 項不是數字（字串、日期）→ Rejected「/X 的值不是數字，不能 delta」；上游的日期加毫秒不做，列已知限制。

## 解析層（`parse.rs`）
- `parse_updates` 改回傳「有效 patches＋拒收記錄」，各帶來源索引，保留原順序與逐物件救援（整段 JSON 陣列解析失敗才退回逐個掃平衡物件）。
- `"op":"add"` 當 insert。
- move 目的地精確照上游 `path ?? to`：`path` 缺或是 null 才取 `to`；兩者都有時 `path` 優先；`path` 不是字串（也不是 null）→ 拒收。
- 拒收條件（Rejected，訊息指出缺哪個欄位或列出可用 op：replace、delta、insert、remove、move；抓得到 path／to／from 就寫進句子，同句去重後 GM 分得出是哪筆）：陣列元素不是物件、缺 op 或 op 不是字串、未知 op、缺 path 或 path 不是字串、move 缺來源或目的地。
- 區塊內容（`strip_analysis`、取 JSONPatch 段、剝圍欄之後）是空的 → 什麼都不記。整段解析成 JSON 陣列（含空 `[]`）→ 照逐物件處理，不發區塊 note（`[]` 照常清空上一輪 notes）。
- 整段不是 JSON 陣列 → 掃平衡物件救援：
  - 平衡但 JSON 解析失敗的物件 → 一條「本輪更新區塊有 N 筆格式壞掉未套用（JSON 語法錯誤）：{抓得到的 `"path":"…"`，否則物件開頭約 30 字}[；其餘已處理（被擋下的另列回饋），已套用的不要重送]。」，排在第一個壞物件處；後半句只在同區塊有其他有效物件且沒有未收尾段落時加。不說「其餘已套用」：有效物件之後仍可能在套用時被擋。
  - 結尾有沒收尾的段落（缺 `}`、字串少收引號）→ 另一條，排在區塊尾：「本輪更新區塊從 {同上標示} 起格式壞掉（JSON 語法錯誤，多半是字串少了收尾引號），[前面的已處理（被擋下的另列回饋）；]這筆起到結尾的更新都未套用，請重送。」——字串少收引號會吞掉之後所有物件（實測 Haiku 清單欄尾端寫成 `\"}`，之後六筆全沒套用）。
  - 一個 `{…}` 都沒有（YAML、條列、`_.set(...)`）→ **沒有可解析句**：「更新區塊裡沒有可解析的 JSONPatch 陣列，整段未套用。」
  - 有效物件照逐物件處理。
- 未重構 MVU 卡若卡片條目教 GM 寫 `_.set`，這桌每輪都會出「沒有可解析」句，屬預期行為（另案補指令式解析前，至少讓 GM 知道沒套用）。

## 其他改動
- `MECHANISM-FORMAT.md` 協定段補 VWD delta 的兩個例外（數字字串照收、明確範圍照夾）；記帳表：`rejected` 補「路徑不存在、對分支增減或替換、在單一欄位底下建子欄位、認不出的 op、格式壞掉的更新物件」；`error` 只留帳本現值壞掉與衍生公式。協定段補 `add` 視同 insert。
- 面板計數：這幾類從 `errors` 移到 `rejected`，前端不改；舊記帳檔照舊顯示。已遊玩桌升級後只影響之後的回合。

## 測試（cargo）
- 改既有：`delta_on_missing_path_is_a_hard_error`、`remove_on_missing_path_is_a_hard_error`（`apply/tests.rs:218`、`:257`）改名改斷言；`apply/tests.rs:440` 註解更新。
- 各類一條：#1／#5／#10／#11 訊息含路徑與最近分支現有鍵、樹不變；只缺末段帶 insert 半句、中間層缺不帶；`replace /神經狀態/0` → 清單句；#2／#6／#8 分支列子鍵；#8 葉子現值照印；#9／#12 單一欄位句，#12 原值放回。
- 單一欄位句依規則：Number／Pair → delta 句、Text → replace 句、Roll → 骰值句、Derived → 唯讀句且不含「請直接更新」。
- `UpdateMode::Replace` 整棵替換分支照舊成功、無記錄。
- insert 半句只在 replace／delta 出現，remove／move／insert 不帶。
- 列鍵：增量桌 Rare／Derived 子鍵與全 Rare 分支不列；全量桌全列；`_` 鍵不列；樹根「頂層」；長路徑＋多個長鍵的完整 note 斷言 ≤ 240 字元、保留路徑與指示、尾端「等 N 項」；同一不存在前綴多筆只一條並帶筆數。
- Upstream 桌：replace／remove／move 來源不存在 → Rejected、不建立、view 不動、同批後續 patch 照常；`/HP/0`（VWD）提示含「只寫新值」不含「整個陣列」，照提示 `replace /HP 55` 重試後持久化（TypedBatch）是 `[55, "說明"]`；一般陣列提示寫整個陣列，照做重試後持久化正確。
- VWD delta：`[50,"說明"]` delta -10 → `[40,"說明"]`、持久化正確、變動標記 -10；delta 給非數字 → Rejected；第 0 項是字串 → Rejected；有 min 規則時夾值。
- 解析層：`add` 當 insert；move 用 `path` 當目的地、只有 `to` 也收；未知 op、缺 path、path 非字串 → Rejected 並在原順序；壞物件 N 筆 → 一條區塊 note、其他物件照收。
- VWD 守門：Roll 規則 VWD 被 delta → Rejected 骰值句、值不變；ReadOnly 規則同（唯讀句）；字串 VWD 的索引路徑提示不含「delta」；Local／Reject 規則的陣列給對應拒收句。
- VWD 數值：0.1＋0.2 → 0.3；大數照 12 位有效數字；同批 replace → delta → replace 每步 view 與持久化正確；結果非有限數時持久化成 null。
- 長度：119 字路徑在句中多次被引用的完整 note 斷言 ≤ 240 字元、路徑只完整出現一次、指示與筆數尾綴都在。
- move 目的地：只有 `to` → 收；`path` 與 `to` 都有 → 用 `path`；`path` 是數字 → 拒收。
- 解析拒收：陣列元素非物件、缺 op、op 非字串、未知 op、缺 path、path 非字串、move 缺 from 各一條，訊息正確且在原順序。
- 壞 JSON：中間一筆缺 `}` → 前面有效物件照收、計數 note 帶 path 與「不要重送」；結尾引號未閉合 → 「從 … 起格式壞掉…請重送」句、不含「其餘已套用」；平衡壞物件＋未收尾段落 → 兩條；`_.set('HP', 5);`（不含大括號）／YAML 內容 → 「沒有可解析的 JSONPatch 陣列」；`[]` → 無記錄且清空舊 notes；只有 `<Analysis>` → 無 note。
- 非有限 delta：`"NaN"`、`"inf"` 對數字欄與 `[50,"說明"]` 都 Rejected、值不動。
- 路徑跳脫：鍵名含 `/`、`~` 的提示路徑照抄後 `parse_pointer` 拆回原鍵。
- `path: null` 的 move 取 `to`；VWD delta `"5"` 成功；min／max 上下界夾值並有 Clamped 記錄、帳本是夾後值。
- 順序與去重：解析拒收夾在兩筆套用錯誤中間時 notes 照來源順序；同一不存在前綴 3 筆 → 一條、印前綴而非完整路徑、尾綴「本輪 3 筆未套用：」帶末段鍵名、帳本 3 筆；跳動／衍生記錄排後。
- 非接管桌沒有 scope_record 時 notes 也在全部記錄收齊後重建。
- 維持 Error：#3、#4、#7、衍生公式算不出來 → 不進 notes。
- 開場白：非接管桌開場白錯路徑 → 第 1 回合 notes 帶出；接管桌開場白不產生。
- 端到端（`feedback_e2e_tests.rs`）：全量／增量桌 × API／CLI 兩路，錯路徑 → 下一輪 GM 尾段有回饋，修正後清空。
- 驗收輪補：`["劍","盾"]` 被 /0、/1、insert /-、remove 打到時提示含「只會改第 0 項（目前「劍」）」不含「只寫新值」，照提示 replace 只換第 0 項；壞 JSON＋不存在路徑 delta 同區塊兩條回饋不矛盾；長路徑非數字 VWD 與非法 delta 不超過 240；同層新鍵合併；半值捨入。
- `npm run verify` 12 步綠。

## 驗證計畫
1. 零 AI：上述 cargo 測試。
2. 真模型（Haiku，2 回合）：非接管重構桌（非露骨測試卡），開場白帶一筆錯路徑 replace 與一筆對分支 delta，看第 1 回合尾段有回饋、GM 是否改對。只作觀察，不擋結案。

## 已知限制
- 第 1 次仍會錯，靠下一輪修正；回饋上限 5 條，錯誤多時後面的要等下一輪。
- 列鍵不扣不在場角色分支。
- VWD 第 0 項是日期時不做上游的加毫秒。
- 陣列索引路徑只回饋不支援（另立案）。
- 區塊內容是純說明文字（如「本輪無變化」）也會出「沒有可解析」句〔模型判斷·未裁決〕。

- 恰好兩個字串的清單（`["劍","盾"]`）照上游被當 [值, 說明]，整列 replace 只會改第 0 項、新陣列會巢狀（上游同）；交 mvu-array-index-path 一併處理。
- VWD delta 的 12 位有效數字：上游中點取較大者，這裡半值取偶，只在二進位值恰好落在中點時差最後一位。
- 真模型只在介面接管桌驗過；非接管重構桌的回饋鏈未實測（單元與端到端測試涵蓋）。
## 範圍外〔模型判斷·未裁決〕
- remove／move 不看欄位規則（可刪掉唯讀、骰值欄）：缺的是守門不是回饋，另立案。
- 英文桌回饋英文化。
- **未重構卡教 GM 寫 `_.set(...)`**：Rust 只解析 JSONPatch（`parse.rs`），沒有指令式解析。`[mvu_update]` 的欄位規則表會被接管不送模型，但抽不出規則的同前綴說明條目（如「變量輸出格式」）照原卡啟停（`import/mechanism.rs:323`），若它教 `_.set`，GM 照寫會被整段靜默略過；卡片 iframe 的 `parseMessage` 只在卡片腳本自己呼叫時跑，不處理 GM 回覆，沒有其他路徑接住。是缺口，已回報主線另立案；本案的「沒有可解析的 JSONPatch 陣列」區塊 note 至少讓 GM 知道整段沒套用。

## 驗證結果（2026-10-12）
- verify 12 步綠（cargo test 全過）。
- 真模型（claude CLI＋Haiku，測試通道）：複製北境驿站介面接管桌（`scratchpad/mef/root`，非接管重構桌改 `refactor_mode` 後 GM 改寫狀態圍欄、不走 UpdateVariable，不具代表性，改用接管桌；回饋路徑兩者相同）。
  - 第 1 回合前注入三條回饋（`/地点` 不存在、對 `/状态栏/驿站物资` 下 delta、`/状态栏/人物列表/0`）：GM 全用正確路徑（`/状态栏/地点`、子欄 `粮草`／`马匹`、清單整串 replace）。
  - 同回合 GM 把清單欄字串尾端寫成 `\"}`（少收引號），之後六筆全被吞掉；原「其餘已套用，不要重送」措辭因此錯誤，已改成「這筆起到結尾的更新都未套用，請重送」（見解析層）。
  - 第 2 回合帶新回饋：GM 重送人物列表（引號正確）、四個選項與粮草，全收、回饋清空。
