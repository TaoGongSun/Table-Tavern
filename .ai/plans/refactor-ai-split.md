# refactor_ai.rs 拆進 refactor_ai/：依賴複核與切線定案

基準：`chatgpt-collab` HEAD `cc8c65e331e438aafd87913ffd78dcebeef0ecc3`，`src-tauri/src/refactor_ai.rs` blob `9c5cf7a35b33e31cca906728f7b181a193cfaa81`。原檔 2764 行；production 本體 1–1810，`#[cfg(test)] mod tests` 自 1811 起。

本案完全沿用 `mechanism/` 的拆法：**純搬家、production body 逐 byte 不動、只做 module plumbing 所需 visibility/import 調整、`mod.rs` 只當 facade、零呼叫端的 re-export 不掛**。本文件只做依賴複核與下一階段施工定案；本輪不改任何 `.rs`。

行號均指上述基準 blob。表內 derive 類型的區間把 `#[derive(...)]` 算進 item；`impl` 另列，不算進 pub/private 統計。

## 1. Production 頂層 item 盤點

總計 **94 個 top-level item**：**34 個 `pub`、58 個 private、2 個 `impl`（本身無 visibility）**。

| # | item | 種類 | 可見度 | 行號 |
|---:|---|---|---|---:|
| 1 | `assemble_card_context` | fn | pub | 40–97 |
| 2 | `EntrySpan` | struct | pub | 106–111 |
| 3 | `segment_spans` | fn | pub | 116–164 |
| 4 | `mark_entry_spans` | fn | private | 168–176 |
| 5 | `format_worldbook_entry` | fn | private | 178–198 |
| 6 | `entry_full_text` | fn | pub | 201–210 |
| 7 | `PrescanSignal` | struct | pub | 218–226 |
| 8 | `daily_style_regex` | fn | private | 230–236 |
| 9 | `template_var_regex` | fn | private | 241–246 |
| 10 | `html_tag_regex` | fn | private | 248–253 |
| 11 | `percent_regex` | fn | private | 255–258 |
| 12 | `prescan_worldbook` | fn | pub | 265–310 |
| 13 | `SYSTEM_PREAMBLE` | const | private | 316–334 |
| 14 | `system_message` | fn | private | 336–343 |
| 15 | `known_fields_line` | fn | private | 347–356 |
| 16 | `SURVEY_BODY` | const | private | 358–450 |
| 17 | `signals_line` | fn | private | 454–469 |
| 18 | `INTERFACE_MODE_BODY` | const | private | 473–480 |
| 19 | `CHARACTERS_MODE_BODY` | const | private | 482–488 |
| 20 | `mode_body` | fn | private | 490–496 |
| 21 | `survey_messages` | fn | pub | 500–517 |
| 22 | `RECOMMEND_BODY` | const | private | 521–532 |
| 23 | `recommend_messages` | fn | pub | 534–544 |
| 24 | `RefactorRecommendOutcome` | struct | pub | 550–560 |
| 25 | `parse_recommend` | fn | pub | 562–586 |
| 26 | `person_body` | fn | private | 590–609 |
| 27 | `INTERFACE_STATE_RULES` | const | private | 612–621 |
| 28 | `INTERFACE_SHELL_RULES` | const | private | 626–640 |
| 29 | `INTERFACE_UPDATE_RULES` | const | private | 646–660 |
| 30 | `MECHANISM_FIELD_SCHEMA` | const | private | 663–679 |
| 31 | `MECHANISM_TRIGGER_SCHEMA` | const | private | 682–696 |
| 32 | `EntryKind` | enum | pub | 700–706 |
| 33 | `impl EntryKind` | impl | n/a | 708–718 |
| 34 | `expand_messages` | fn | pub | 720–756 |
| 35 | `person_expand_messages` | fn | pub | 759–782 |
| 36 | `GroupKind` | enum | pub | 789–793 |
| 37 | `impl GroupKind` | impl | n/a | 795–812 |
| 38 | `absorb_body` | fn | private | 816–842 |
| 39 | `absorb_messages` | fn | pub | 845–866 |
| 40 | `group_materials_block` | fn | private | 869–875 |
| 41 | `GROUP_LARGE_MATERIAL_THRESHOLD` | const | private | 879 |
| 42 | `group_body` | fn | private | 881–930 |
| 43 | `group_messages` | fn | pub | 934–957 |
| 44 | `Block` | struct | private | 964–968 |
| 45 | `parse_blocks` | fn | private | 973–992 |
| 46 | `trim_heading_prefix` | fn | private | 995–1008 |
| 47 | `match_marker` | fn | private | 1010–1027 |
| 48 | `join_trim` | fn | private | 1029–1031 |
| 49 | `strip_json_fence` | fn | private | 1034–1042 |
| 50 | `strip_html_fence` | fn | private | 1046–1058 |
| 51 | `RefactorSurveyPerson` | struct | pub | 1066–1079 |
| 52 | `RefactorEntryVerdict` | struct | pub | 1084–1092 |
| 53 | `RefactorSpanRoute` | struct | pub | 1097–1111 |
| 54 | `RefactorSplitGroup` | struct | pub | 1114–1121 |
| 55 | `RefactorSurveyOutcome` | struct | pub | 1123–1152 |
| 56 | `parse_uid_line` | fn | private | 1156–1163 |
| 57 | `is_affirmative` | fn | private | 1167–1170 |
| 58 | `parse_interface_line` | fn | private | 1174–1181 |
| 59 | `locate_fields` | fn | private | 1187–1198 |
| 60 | `field_value` | fn | private | 1204–1217 |
| 61 | `PERSON_FIELD_KEYS` | const | private | 1219 |
| 62 | `parse_person_line` | fn | private | 1225–1257 |
| 63 | `parse_uid_list` | fn | private | 1259–1265 |
| 64 | `is_valid_span_ref` | fn | private | 1268–1275 |
| 65 | `parse_span_list` | fn | private | 1277–1283 |
| 66 | `split_first_token` | fn | private | 1287–1293 |
| 67 | `find_trailing_field` | fn | private | 1297–1306 |
| 68 | `parse_rule_field` | fn | private | 1308–1314 |
| 69 | `ENTRY_ACTIONS` | const | private | 1316 |
| 70 | `ENTRY_FIELD_KEYS` | const | private | 1317 |
| 71 | `parse_entry_line` | fn | private | 1322–1343 |
| 72 | `SPLIT_ROUTES` | const | private | 1345–1353 |
| 73 | `SPLIT_FIELD_KEYS` | const | private | 1354 |
| 74 | `parse_split_line` | fn | private | 1360–1396 |
| 75 | `GROUP_FIELD_KEYS` | const | private | 1398 |
| 76 | `parse_group_line` | fn | private | 1403–1429 |
| 77 | `parse_field_line` | fn | private | 1432–1439 |
| 78 | `parse_survey` | fn | pub | 1441–1516 |
| 79 | `normalize_survey_for_mode` | fn | pub | 1523–1527 |
| 80 | `RefactorExpandOutcome` | struct | pub | 1531–1537 |
| 81 | `parse_character_body` | fn | private | 1539–1569 |
| 82 | `RefactorPersonExpandOutcome` | struct | pub | 1573–1579 |
| 83 | `parse_person_expand` | fn | pub | 1583–1611 |
| 84 | `parse_interface_expand` | fn | private | 1617–1642 |
| 85 | `parse_json_block` | fn | private | 1645–1655 |
| 86 | `parse_expand` | fn | pub | 1657–1668 |
| 87 | `RefactorEntryMeta` | struct | pub | 1674–1682 |
| 88 | `RefactorNewEntry` | struct | pub | 1686–1703 |
| 89 | `RefactorRewriteOutcome` | struct | pub | 1706–1712 |
| 90 | `RefactorAbsorbOutcome` | struct | pub | 1716–1724 |
| 91 | `parse_absorb` | fn | pub | 1728–1742 |
| 92 | `parse_group` | fn | pub | 1747–1786 |
| 93 | `span_placeholder_regex` | fn | private | 1790–1795 |
| 94 | `expand_span_placeholders` | fn | pub | 1801–1809 |

### 統計

- `pub` top-level item：**34**。
- private top-level item：**58**。
- `impl`：**2**（`EntryKind`、`GroupKind`；不重複計入 pub/private）。

## 2. 實際依賴 DAG

只看 production 裡的真實符號引用／呼叫，不拿行號相鄰當依據。外部 crate/module 呼叫（`data::*`、`mechanism::*`、serde、regex、`RefactorCharacter`／`RefactorInterface` 等）不是本檔 sibling 邊，以下只列 `refactor_ai` 內部 DAG。

### context / span / prescan

```text
segment_spans
  ↑
mark_entry_spans
  ↑
format_worldbook_entry ← assemble_card_context
  ↑
entry_full_text

segment_spans ───────────────┐
daily_style_regex ───────────┤
template_var_regex ──────────┤
html_tag_regex ──────────────┼→ prescan_worldbook
percent_regex ───────────────┘
```

精確邊：

- `assemble_card_context → format_worldbook_entry`
- `mark_entry_spans → segment_spans`
- `format_worldbook_entry → mark_entry_spans`
- `entry_full_text → format_worldbook_entry`
- `prescan_worldbook → segment_spans`
- `prescan_worldbook → daily_style_regex | template_var_regex | html_tag_regex | percent_regex`

四個 regex helper 都是 leaf；沒有 helper 回頭呼叫 prescan/span，無環。

### prompt builders

```text
SYSTEM_PREAMBLE → system_message

INTERFACE_MODE_BODY / CHARACTERS_MODE_BODY → mode_body
PrescanSignal → signals_line
SURVEY_BODY + mode_body + signals_line + system_message → survey_messages
RECOMMEND_BODY + system_message → recommend_messages

INTERFACE_STATE_RULES ────────┐
INTERFACE_SHELL_RULES ────────┤
INTERFACE_UPDATE_RULES ───────┤
MECHANISM_FIELD_SCHEMA ───────┼→ expand_messages
known_fields_line ────────────┤
system_message ───────────────┘
person_body + system_message → person_expand_messages

MECHANISM_FIELD_SCHEMA ─┐
MECHANISM_TRIGGER_SCHEMA ┼→ absorb_body → absorb_messages
known_fields_line ───────┤
system_message ──────────┘

group_materials_block ──────────────┐
GROUP_LARGE_MATERIAL_THRESHOLD ─┐   │
MECHANISM_FIELD_SCHEMA ─────────┼→ group_body ─┤
MECHANISM_TRIGGER_SCHEMA ───────┘   │
known_fields_line ──────────────────┤→ group_messages
system_message ─────────────────────┘
```

`system_message` 是所有 AI 階段共同扇出點；這是 prompt cache 的同 system 不變量，不應複製到各檔。`known_fields_line` 被 `expand_messages`、`absorb_messages`、`group_messages` 三路共用。`MECHANISM_FIELD_SCHEMA` 同時被 interface-shell、absorb、mechanism-group 使用；`MECHANISM_TRIGGER_SCHEMA` 被 absorb/group 共用。

### parser common

```text
trim_heading_prefix ─┐
match_marker ────────┼→ parse_blocks

join_trim ───────────────┐
strip_json_fence ────────┼→ parse_json_block
```

- `parse_blocks → trim_heading_prefix, match_marker`
- `parse_json_block → join_trim, strip_json_fence`
- `strip_html_fence` 是 interface parser leaf helper。

### survey parser

```text
parse_uid_line + is_affirmative → parse_interface_line

locate_fields + field_value + PERSON_FIELD_KEYS
parse_uid_list + is_affirmative + parse_span_list
  → parse_person_line

is_valid_span_ref → parse_span_list
find_trailing_field → parse_rule_field

parse_uid_line + locate_fields + field_value
ENTRY_ACTIONS + ENTRY_FIELD_KEYS → parse_entry_line

locate_fields + field_value + SPLIT_FIELD_KEYS + SPLIT_ROUTES
is_valid_span_ref + split_first_token + parse_rule_field + find_trailing_field
  → parse_split_line

locate_fields + field_value + GROUP_FIELD_KEYS + parse_span_list
  → parse_group_line

parse_blocks
parse_person_line / parse_interface_line / parse_entry_line
parse_split_line / parse_group_line / parse_field_line
  → parse_survey
```

`normalize_survey_for_mode` 只讀寫 `RefactorSurveyOutcome`，不回呼 parser。

### result parser / placeholder

```text
parse_blocks + join_trim → parse_character_body
parse_blocks + parse_character_body → parse_person_expand

parse_blocks + join_trim + strip_json_fence + strip_html_fence + parse_json_block
  → parse_interface_expand → parse_expand

parse_blocks + parse_json_block → parse_absorb
parse_blocks + join_trim + parse_json_block + GroupKind::as_str → parse_group

span_placeholder_regex → expand_span_placeholders
```

`parse_recommend` 是獨立 leaf parser；`EntryKind::parse`、`GroupKind::parse` 也是 leaf。

### 環檢查結論

**沒有互相依賴的環（SCC > 1 為 0）**。幾個高扇出共用節點是 `segment_spans`、`system_message`、`known_fields_line`、`parse_blocks`、`parse_json_block`；正確切法是把這些共用節點放在下層共用模組，讓依賴單向往下，不要按原檔行號硬切。

## 3. 切線定案

### Production：9 個 implementation 檔 + 純 facade `mod.rs`

| 檔案 | 放入 item | 預估 production 行數 |
|---|---|---:|
| `types.rs` | `EntrySpan`、`PrescanSignal`、`RefactorRecommendOutcome`、`EntryKind`＋impl、`GroupKind`＋impl、`RefactorSurveyPerson`、`RefactorEntryVerdict`、`RefactorSpanRoute`、`RefactorSplitGroup`、`RefactorSurveyOutcome`、`RefactorExpandOutcome`、`RefactorPersonExpandOutcome`、`RefactorEntryMeta`、`RefactorNewEntry`、`RefactorRewriteOutcome`、`RefactorAbsorbOutcome` | ~210 |
| `context.rs` | `assemble_card_context`、`segment_spans`、`mark_entry_spans`、`format_worldbook_entry`、`entry_full_text`、四個 prescan regex helper、`prescan_worldbook` | ~265 |
| `prompt_common.rs` | `SYSTEM_PREAMBLE`、`system_message`、`known_fields_line`、三個 `INTERFACE_*_RULES`、`MECHANISM_FIELD_SCHEMA`、`MECHANISM_TRIGGER_SCHEMA` | ~135 |
| `survey.rs` | `SURVEY_BODY`、`signals_line`、兩個 mode body、`mode_body`、`survey_messages`、`RECOMMEND_BODY`、`recommend_messages` | ~225 |
| `expand.rs` | `person_body`、`expand_messages`、`person_expand_messages` | ~95 |
| `rewrite.rs` | `absorb_body`、`absorb_messages`、`group_materials_block`、`GROUP_LARGE_MATERIAL_THRESHOLD`、`group_body`、`group_messages` | ~125 |
| `parse_common.rs` | `Block`、`parse_blocks`、`trim_heading_prefix`、`match_marker`、`join_trim`、`strip_json_fence`、`strip_html_fence`、`parse_json_block` | ~105 |
| `survey_parse.rs` | `parse_recommend`、1156–1439 全部 survey line helper/常數、`parse_survey`、`normalize_survey_for_mode` | ~400 |
| `result_parse.rs` | `parse_character_body`、`parse_person_expand`、`parse_interface_expand`、`parse_expand`、`parse_absorb`、`parse_group`、`span_placeholder_regex`、`expand_span_placeholders` | ~185 |

估算含各檔必要 `use`／空行，不含測試。總量與原 production 1810 行相符，差額主要是原檔總註解／分隔線與拆檔新增 import。

### 為什麼共用型別放 `types.rs`，不留 `mod.rs`

實際依賴不是「某一段自己的型別」：

- `EntrySpan`：`context.rs` 產生，`refactor_assemble.rs` 直接使用。
- `PrescanSignal`：`context.rs` 產生，`survey.rs` 的 public signature 使用，原檔 tests 也直接建構。
- `EntryKind`：`expand.rs` 與 `result_parse.rs` 都要，`commands/refactor.rs` 也直接 `EntryKind::parse`／用 variant。
- `GroupKind`：`rewrite.rs` 與 `result_parse.rs` 都要，`commands/refactor.rs` 直接 `GroupKind::parse`。
- survey 五種資料型別＋`RefactorSurveyOutcome`：`survey_parse.rs` 產生，`refactor_assemble.rs`／`commands/refactor.rs` 消費。
- outcome/new-entry 類型：`result_parse.rs` 產生，`commands/refactor.rs`、`refactor_assemble.rs`、`refactor.rs` 消費。

以定案切線計，`types.rs` 會被 **6 個 implementation 模組**直接 import（`context`、`survey`、`expand`、`rewrite`、`survey_parse`、`result_parse`），另有三個外部 Rust 消費檔。若把這批型別塞在 `mod.rs`，facade 又會變成實作層，違反上一案已拍板的 `mod.rs` 純 facade 原則。因此 **型別集中 `types.rs`**。

### 模組 DAG

```text
types
├─ context
├─ survey       ─┐
├─ expand       ├─→ prompt_common
├─ rewrite      ┘
├─ survey_parse ─→ parse_common
└─ result_parse ─→ parse_common

survey / expand / rewrite → prompt_common
survey_parse / result_parse → parse_common
```

箭頭表示「左邊依賴右邊」。沒有 sibling 雙向邊。

跨檔 private helper 在施工時只做必要的 `pub(super)`：典型是 `system_message`、`known_fields_line`、`parse_blocks`、`join_trim`、`strip_json_fence`、`strip_html_fence`、`parse_json_block`。不改函式 body。

### 測試搬法

1811–2764 的 tests 不參與 production DAG 定線。沿用 `mechanism/`：**測試跟著 owner implementation 搬**，不要保留一個 954 行的大 `tests.rs`。例如 span/prescan 測試進 `context.rs`，survey/recommend prompt 測試進 `survey.rs`，survey parser 測試進 `survey_parse.rs`，interface/person/absorb/group parser 測試進 `result_parse.rs`。只有 `all_stage_system_messages_are_byte_identical_for_same_context` 這種跨模組不變量可留一個很小的 root `#[cfg(test)]` integration test module（或放 `mod.rs` 下的 `tests.rs`），但 `mod.rs` production 仍只有 mod 宣告＋re-export。

下一階段仍需像 mechanism 案一樣做 test leaf 名稱 multiset、production body hash/byte 驗收；本文件不在這輪執行施工。

## 4. 對外呼叫端清查

### 搜尋口徑

逐一搜尋 34 個 production `pub` top-level item。Rust 呼叫端只算 `src-tauri/src/`：

- 「其他檔」：函式呼叫、型別註記／建構、enum/associated fn/variant 的直接符號引用；測試若位在**其他 `.rs`** 仍記在其他檔並標 `(test)`。
- 「同檔 tests」：只算目前 `refactor_ai.rs` 1811–2764 對該 symbol 的直接引用。
- 宣告本身、doc/comment、`.ai/` 文件、TS 端同名 mirror interface 不算 Rust 呼叫端。
- `PrescanSignal` 這類型別若其他檔只透過型別推導承接 `prescan_worldbook` 結果、沒有寫出 symbol 名，不算「直接引用」。

### 有呼叫端：33 個

| pub item | `src-tauri/src/` 其他檔直接引用 | 同檔 tests 直接引用 |
|---|---|---|
| `assemble_card_context` | `commands/refactor.rs` | 有 |
| `EntrySpan` | `refactor_assemble.rs` | 有 |
| `segment_spans` | `refactor_assemble.rs` | 有 |
| `entry_full_text` | `commands/refactor.rs` | 無 |
| `PrescanSignal` | 無 | **有**（直接 struct literal） |
| `prescan_worldbook` | `commands/refactor.rs`、`refactor_assemble.rs` | 有 |
| `survey_messages` | `commands/refactor.rs` | 有 |
| `recommend_messages` | `commands/refactor.rs` | 有 |
| `RefactorRecommendOutcome` | `commands/refactor.rs`（command return type） | 無 |
| `parse_recommend` | `commands/refactor.rs` | 有 |
| `EntryKind` | `commands/refactor.rs` | 有 |
| `expand_messages` | `commands/refactor.rs` | 有 |
| `person_expand_messages` | `commands/refactor.rs` | 有 |
| `GroupKind` | `commands/refactor.rs` | 有 |
| `absorb_messages` | `commands/refactor.rs` | 有 |
| `group_messages` | `commands/refactor.rs` | 有 |
| `RefactorSurveyPerson` | `refactor_assemble.rs` `(test)` | 無直接 symbol 引用 |
| `RefactorEntryVerdict` | `refactor_assemble.rs` | 無直接 symbol 引用 |
| `RefactorSpanRoute` | `refactor_assemble.rs` | 無直接 symbol 引用 |
| `RefactorSplitGroup` | `refactor_assemble.rs` `(test)` | 無直接 symbol 引用 |
| `RefactorSurveyOutcome` | `commands/refactor.rs`、`refactor_assemble.rs` | 無直接 symbol 引用 |
| `parse_survey` | `commands/refactor.rs` | 有 |
| `normalize_survey_for_mode` | `commands/refactor.rs` | 有 |
| `RefactorExpandOutcome` | `commands/refactor.rs`（command return type） | 無 |
| `RefactorPersonExpandOutcome` | `commands/refactor.rs`（command return type） | 無 |
| `parse_person_expand` | `commands/refactor.rs` | 有 |
| `parse_expand` | `commands/refactor.rs` | 有 |
| `RefactorEntryMeta` | `refactor_assemble.rs`、`refactor.rs` `(test)` | 無直接 symbol 引用 |
| `RefactorNewEntry` | `commands/refactor.rs`、`refactor_assemble.rs`、`refactor.rs` | 無直接 symbol 引用 |
| `RefactorRewriteOutcome` | `commands/refactor.rs` | 無直接 symbol 引用 |
| `parse_absorb` | `commands/refactor.rs` | 有 |
| `parse_group` | `commands/refactor.rs` | 有 |
| `expand_span_placeholders` | `commands/refactor.rs` | 有 |

特別注意：`prescan_worldbook` 不是只有 survey command 用；`refactor_assemble.rs` 的涵蓋稽核也直接呼叫，因此它有 **2 個其他 `.rs` 呼叫檔**。

### 零呼叫端：1 個

| pub item | 其他 `src-tauri/src/` | 同檔 tests | 拆後 facade |
|---|---|---|---|
| `RefactorAbsorbOutcome` | 0 | 0（tests 只透過 `parse_absorb` 的推導結果存取欄位，沒有直接寫型別名） | **不 `pub use`** |

因此按本輪要求，拆檔後 `RefactorAbsorbOutcome` 留在 `types.rs` 供 `parse_absorb` 回傳型別使用，但 **`mod.rs` 不掛 `pub use types::RefactorAbsorbOutcome`**。

`PrescanSignal` 與它不同：其他 `.rs` 雖沒有直接寫型別名，但原檔 tests 會直接建構，故在「全 repo＋同檔 tests」口徑不是零呼叫端；不要把兩者混為一談。

### facade 原則

`mod.rs` 只做：

1. `mod context; mod expand; mod parse_common; mod prompt_common; mod result_parse; mod rewrite; mod survey; mod survey_parse; mod types;`
2. `pub use ...` 保住目前有呼叫端的 `crate::refactor_ai::<name>` 路徑。
3. **唯一確定不掛的既有 pub re-export：`RefactorAbsorbOutcome`。**

private helper 只在 sibling 需要時升成 `pub(super)`，不做 facade re-export。production function/const body 不重寫、不順手整理 prompt、不改 regex、不改 parser 容錯。

## 施工紅線（下一階段）

- 只搬 `refactor_ai.rs` → `refactor_ai/`；呼叫端路徑靠 facade 保持，不順手改 `commands/refactor.rs`／`refactor_assemble.rs`／`refactor.rs`。
- production body 逐 byte 不動；允許的差異只有 `use`、`mod`、`pub(super)`/re-export 等 module plumbing。
- `mod.rs` 不塞型別、不塞 helper、不塞 prompt 常數。
- 拆後依賴方向須維持本文件 DAG；若編譯時出現雙向 sibling 依賴，代表切線或 visibility 有誤，不以互相 `pub(super)` 硬補成環。
- 零呼叫端 `RefactorAbsorbOutcome` 不掛 facade。
