# mechanism.rs 拆進 mechanism/

`src-tauri/src/mechanism.rs` 2870 行（本體 1395／同檔測試 1475），transport-split 之後的次大檔。做法、白名單與機械驗收整套沿用 [transport-split](transport-split.md)（其源頭為 [data-split](data-split.md)）：純搬家、production body 逐 byte 不動、`mod.rs` 只當 facade、零呼叫端的 re-export 不掛。本檔只記本案特有的切線與待複核項。

按**總行數**是本檔最大；按**本體行數**則是 `refactor_ai.rs`（本體 1810／總 2764）最大。2026-09-03 使用者拍板先做 mechanism，refactor_ai 留待下一案。

## 現況盤點（2026-09-03）

本體 56 個頂層 item，對外 `pub` 17 個：`PatchOp`、`Patch`、`RecordKind`、`Record`、`Outcome`、`TriggerOutcome`、`LedgerEntry`、`Ledger`、`parse_updates`、`apply_updates`、`rule_for_path`、`reroll`、`recompute_derived`、`evaluate_triggers`、`apply_block`、`append_log`、`read_ledger`。其餘 39 個是同檔 private helper，拆檔後需要多少 `pub(super)` 由編譯器決定。

## 切線草案（本體 1395 行 → 7 檔）**待開工複核**

| 檔 | 內容（現 mechanism.rs 行號） |
|---|---|
| `parse.rs` | `parse_updates`、`strip_analysis`、`extract_json_patch_section`、`strip_code_fences`、`scan_balanced_objects`、`value_to_patch`、`parse_pointer`（81–269） |
| `apply.rs` | `apply_updates`、`apply_one`、`signed_delta_mark`、`readonly_violation`、`apply_delta`／`apply_replace`／`replace_existing`／`apply_insert`／`apply_remove`／`apply_move`（270–663） |
| `tree.rs` | `build_notes`、`leaf_value`、`insert_node`、`next_free_key`、`take_node`、`json_to_node`、`format_json_number`、`format_num`、`split_pair`、`value_as_f64`（664–804） |
| `rules.rs` | `rule_for_path`、`rule_for`、`wildcard_rule`（805–854） |
| `derive.rs` | `reroll`／`reroll_branch`／`random_int_in_range`、`recompute_derived`、`collect_derived_targets`、`tree_lookup`（855–971） |
| `triggers.rs` | `TriggerOutcome`、`evaluate_triggers`、`PathValue`、`resolve_path`、`condition_holds`、`leaf_at`、`current_number`、`resolve_state_placeholders`（972–1125） |
| `ledger.rs` | `old_field_value`、`numeric_value`、`detect_jumps`、`apply_block`、`append_log`、`LedgerEntry`／`Ledger`／`ledger_rank`、`read_ledger`（1126–1395） |

型別 `PatchOp`／`Patch`／`RecordKind`／`Record`／`Outcome`＋兩個 threshold 常數（15–80）被多檔共用，放 `types.rs` 或直接留 `mod.rs`，開工時看實際 import 數決定。

`apply_block` 是本檔的總入口（解析→套用→衍生→觸發→跳變偵測一路串起來），放 `ledger.rs` 是因為它與 `detect_jumps`／`append_log` 同段；若複核發現它依賴面太廣，改獨立成 `block.rs`。

## 開工前必做

1. **複核依賴方向**：transport 那案開工前以為的三段式切線是假的（下游函式排在檔案前段）。本案的行號分段同樣只是視覺印象，要實際查呼叫關係、畫出 DAG、確認無 sibling 環，再定案。
2. **抓拆前 baseline**：production 頂層 item 清單（含可見度）、對外簽名、全庫 test leaf 名稱 multiset、mechanism 測試函式 body hash。驗收就是比對這四份。
3. 確認 `mechanism` 目前對外被誰用（`commands/`、`lanes.rs`、`transport/`），facade 要保住的路徑以實際呼叫端為準。
