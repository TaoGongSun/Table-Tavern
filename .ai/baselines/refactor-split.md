# refactor.rs 拆分施工前 baseline

基準日期：2026-09-05  
工作分支：`refactor-split`  
基準 `main`：`1543e3ddb5033a2673486e5d256a3b999b3ca3c2`  
施工前 branch HEAD：`fe6a618687c408386259140df8ba755bc3d72e87`  
`src-tauri/src/refactor.rs` blob：`860e4adc9e4f8fbe0b70f26e94306bdaefc61fb6`

本文件是 `.ai/plans/refactor-split.md` 工作段 A 的施工前基準。複核時工作分支相對 `main` ahead 1 / behind 0，唯一差異是立案書；因此原始 `refactor.rs` blob 未漂移。

## 1. Raw body 基準口徑

沿用 `import-split` 已採用的 connector-only baseline 口徑：目前沒有可執行 repo filesystem，因此**不偽造本地 SHA-256 / per-item body hash**。Production 與 tests 的逐 byte source of truth 是 immutable git blob `860e4adc…`，搭配下面的 production item manifest 與 test leaf manifest。

結案 integrity 驗收必須直接從這個 blob 抽原 item / test body 對拆後檔案比對；施工環境若能執行腳本，再補算 per-item hash。hash 是機械比對工具，不取代 immutable blob。

允許從 raw body 排除的只有拆檔 plumbing：新增 `use`、module path、事先 ledger 的最小 visibility、facade，以及 test module import / test-support visibility。函式、型別、常數、測試函式 body 不得趁搬家改寫。

## 2. Production top-level item manifest

原 production：1–769，共 **20 個 top-level item = 9 pub + 11 private**。owner 是工作段 A 複核後的施工定案；原檔定位區段沿用立案時從 immutable blob 查得的區段（types 14–146、apply 147–534、interface helpers 535–755、stored mode 756–769），逐 item 邊界以 blob 本身為準，不另猜行號。

| owner | item | kind | 原 visibility |
|---|---|---|---|
| `apply.rs` | `PALETTE` | const | private |
| `types.rs` | `RefactorCharacter` | struct | pub |
| `types.rs` | `RefactorInterface` | struct | pub |
| `types.rs` | `RefactorMechanism` | struct | pub |
| `types.rs` | `RefactorOutcome` | struct | pub |
| `types.rs` | `RefactorSelection` | struct | pub |
| `types.rs` | `RefactorApplySummary` | struct | pub |
| `types.rs` | `RefactorApplyResult` | struct | pub |
| `apply.rs` | `apply` | fn | pub |
| `apply.rs` | `delete_source_entry` | fn | private |
| `apply.rs` | `absorbed_ledger_record` | fn | private |
| `apply.rs` | `absorbed_ledger_record_for_title` | fn | private |
| `interface.rs` | `normalize_interface_paths` | fn | private |
| `interface.rs` | `flatten_leaves` | fn | private |
| `interface.rs` | `unflatten` | fn | private |
| `interface.rs` | `is_empty_value` | fn | private |
| `interface.rs` | `shell_placeholders` | fn | private |
| `interface.rs` | `rebuild_state_fields` | fn | private |
| `interface.rs` | `json_to_state_node` | fn | private |
| `types.rs` | `normalize_stored_mode` | fn | pub |

### 定案切線

```text
src-tauri/src/refactor/
  mod.rs
  types.rs
  apply.rs
  interface.rs
  test_support.rs
  tests/
    mod.rs
    characters.rs
    interface.rs
    mechanism.rs
    entries.rs
```

Production 不再細拆：本體只有 769 行，三個責任群已足夠；維護收益的大宗在把 1524 行單一 tests module 拆開。

## 3. External caller / public surface baseline

code search 複核到的實際 Rust symbol caller：

- `refactor_ai/types.rs`：`RefactorCharacter`、`RefactorInterface`
- `refactor_ai/result_parse.rs`：`RefactorCharacter`、`RefactorInterface`
- `refactor_assemble.rs`：`RefactorCharacter`
- `commands/refactor.rs`：`RefactorOutcome`、`RefactorSelection`、`RefactorApplySummary`、`apply`、`normalize_stored_mode`

`receipts.rs` 目前只有註解提到 `refactor::apply`，不是 symbol caller；`lib.rs` 的 `commands::refactor::...` 是 command 註冊，不是 `crate::refactor` surface caller。

### 對「零直接 caller 不 re-export」的複核修正

立案時把 `RefactorMechanism`、`RefactorApplyResult` 列為「零直接 caller，可能不從 facade re-export」。工作段 A 複核後**不再把它們當成可直接拔掉的普通零-caller item**：

- `RefactorMechanism` 是公開 `RefactorOutcome.mechanisms` 的元素型別。
- `RefactorApplyResult` 是公開 `apply()` 的回傳型別；`commands/refactor.rs` 雖不寫出型別名稱，仍直接讀它的欄位。

因此施工預設是 **9 個原 `pub` root item 全部保留 `refactor::X` facade path**。這不是為了目錄整齊多掛 API，而是避免公開 signature 的 effective visibility 變窄，同時忠實保留拆前 surface。若實際 compiler / `-Dprivate_interfaces` 證明可更窄，仍以「不改 caller、不新增替代 path」為前提再決定，不能為了省一條 re-export 改 caller。

facade 預期白名單：

1. `RefactorCharacter`
2. `RefactorInterface`
3. `RefactorMechanism`
4. `RefactorOutcome`
5. `RefactorSelection`
6. `RefactorApplySummary`
7. `RefactorApplyResult`
8. `apply`
9. `normalize_stored_mode`

## 4. Production DAG 與 visibility ledger

實際呼叫關係複核後，立案草案成立，沒有 sibling cycle：

```text
apply.rs ─────────────→ types.rs
   └──────────────────→ interface.rs

interface.rs ─────────→ data::{FieldRule, StateNode}
types.rs ─────────────→ data types
   ├──────────────────→ refactor_ai::RefactorNewEntry
   └──────────────────→ refactor_assemble::{RefactorDroppedEntry,
                                           RefactorUnabsorbedItem,
                                           RefactorAuditItem}
```

關鍵跨 sibling 呼叫只有：

- `apply` → `normalize_interface_paths`
- `apply` → `rebuild_state_fields`

其餘 interface helper 都只被 `interface.rs` 自己使用；來源刪除與 ledger helper 都只被 `apply.rs` 自己使用；`types.rs` 不反向依賴任何 refactor sibling implementation。

Production private → `pub(super)` 預期白名單鎖 **2 項**：

1. `interface::normalize_interface_paths`
2. `interface::rebuild_state_fields`

施工不得預先放寬其他 production item。若 compiler 顯示還有第三項，先回寫 ledger 再改。

## 5. Test leaf baseline

原 `#[cfg(test)] mod tests`：771–2294。人工逐段讀 immutable blob，盤點為 **33 支 `#[test]` leaf**。結案以名稱 multiset完全一致、test function body 原文一致為準。

### `tests/characters.rs`：7

- `apply_merges_multi_source_person_deletes_exclusive_entries_and_sets_player_then_undo_restores`
- `apply_rejects_second_player_card_and_writes_nothing`
- `apply_unselected_person_gets_independent_person_entry_selected_persons_source_deleted`
- `apply_partial_group_selection_creates_person_entries_for_the_rest_and_keeps_shared_source`
- `apply_shared_uid_kept_when_not_all_owners_selected`
- `apply_shared_uid_deleted_once_when_all_owners_selected_and_verdict_deletable`
- `apply_shared_uid_kept_without_finish_verdict_even_if_all_owners_selected`

### `tests/interface.rs`：15

- `apply_persists_refactor_mode_and_characters_removes_stale_shell`
- `apply_ignores_invalid_mode_values`
- `normalize_stored_mode_fixes_legacy_case_and_rejects_unknown`
- `apply_interface_rebuilds_dirty_state_and_undo_restores_every_key`
- `apply_characters_mode_skips_interface_and_keeps_sources`
- `normalize_collapses_mirror_branch_with_shell_deciding_canon`
- `normalize_rejects_conflicting_values_and_double_referenced_shell`
- `normalize_without_shell_folds_only_complete_mirror`
- `apply_rejects_conflicting_interface_before_any_write`
- `apply_interface_syncs_new_tree_into_scene_snapshots`
- `apply_interface_with_non_object_state_fields_leaves_tree_unchanged`
- `refactor_interface_deserializes_legacy_json_without_shell_field`
- `apply_interface_with_shell_writes_file_readable_via_data_layer`
- `apply_interface_without_shell_creates_no_shell_file`
- `apply_interface_shell_then_undo_deletes_shell_file`

### `tests/mechanism.rs`：3

- `apply_mechanism_deletes_source_after_recording_absorption`
- `apply_mechanism_deletes_source_with_no_prior_ledger_record`
- `apply_mechanism_then_undo_restores_ledger_to_previous_state`

### `tests/entries.rs`：8

- `apply_disables_whole_entry_drops_and_undo_restores`
- `apply_selected_rewritten_entries_creates_locked_mechanism_merges_rules_logs_and_deletes_shared_source`
- `apply_partially_selected_rewritten_entries_keeps_shared_source`
- `legacy_outcome_without_entries_deserializes_and_applies`
- `apply_writes_refactor_outcome_file_readable_and_round_trips`
- `apply_then_undo_keeps_refactor_outcome_file`
- `undo_removes_new_entries_including_locked`
- `apply_entry_with_meta_preserves_fields_without_meta_uses_defaults`

總數驗算：**7 + 15 + 3 + 8 = 33**。

## 6. Test support / test visibility ledger

原 root tests 共用 helper：

- `NEXT_TEMP_ID`
- `TestRoot`
- `TestRoot::new`
- `TestRoot::path`
- `Drop for TestRoot`
- `seed_entry`
- `character`
- `no_player_selection`
- `apply_recorded`

拆後 `test_support.rs` 只收這組共用夾具，不收任何 production helper。為讓 `tests/*` sibling 使用，允許的 **cfg(test)-only** visibility plumbing：

- `TestRoot` → `pub(super)`
- `TestRoot::new` → `pub(super)`
- `TestRoot::path` → `pub(super)`
- `seed_entry` → `pub(super)`
- `character` → `pub(super)`
- `no_player_selection` → `pub(super)`
- `apply_recorded` → `pub(super)`

`NEXT_TEMP_ID` 與 `Drop` 實作保持 private／原樣。這份 test-only ledger 不算 production API 擴張。

## 7. cfg ledger

拆前只有：

- `#[cfg(test)] mod tests`（771–2294）

拆後預期：

- `mod.rs`：`#[cfg(test)] mod test_support;`
- `mod.rs`：`#[cfg(test)] mod tests;`
- `tests/mod.rs`：只宣告 `characters` / `interface` / `mechanism` / `entries`

不得新增 production cfg 分支。

## 8. 工作段 A 結論

- branch base：**未漂移**，複核時 ahead 1 / behind 0，唯一差異是立案書。
- source blob：**未漂移**，仍是 `860e4adc…`。
- production item：**20 = 9 pub + 11 private**。
- production 切線：`types` / `apply` / `interface`，**DAG 成立、無 sibling cycle**。
- production visibility widen：預期只有 **2 項 `pub(super)`**。
- facade：工作段 A 修正立案時的零-caller假設，施工預設 **9 個原 root pub item 全保留**；`RefactorMechanism` / `RefactorApplyResult` 是公開 signature component，不視為普通死 re-export。
- test leaf：**33 支**，owner 定案為 characters 7 / interface 15 / mechanism 3 / entries 8。
- test support：確定新增，允許 7 個 cfg(test)-only `pub(super)` plumbing 點。
- `.rs` 程式碼：工作段 A **0 修改**。

下一工作段 B：production 純搬家，建立 `refactor/{mod,types,apply,interface}.rs`，先只處理 20 個 production item 與 facade；tests 先不搬，完成後先做可取得的編譯／private-interface 驗證，再進工作段 C。