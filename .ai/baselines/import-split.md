# import.rs 拆分施工前 baseline

基準日期：2026-09-04  
工作分支：`refactor/import-split`  
基準 `main`：`02c7e1196bf246ddbd896e71d6023371c8d74bbb`  
施工前 branch HEAD：`4a782bf583e6443b1f32b854d6e8ea5b3eff08da`  
`src-tauri/src/import.rs` blob：`1332ba62cd1541cdb6edf519bd8fc9b5c2286e84`

本文件是 `.ai/plans/import-split.md` 工作段 A 的施工前基準。開工複核時 `main` 仍停在立案基準，工作分支相對 `main` ahead 1 / behind 0，唯一差異是立案書；因此原始 `import.rs` blob 未漂移。

## 1. Raw body 基準口徑

這輪透過 GitHub connector 直接讀 immutable blob，沒有可執行 repo filesystem；因此**不偽造本地 SHA-256**。Production 與 tests 的逐 byte 原文基準就是上面的 immutable git blob `1332ba62…`，搭配下面的 item／test leaf manifest。結案驗收時從這個 blob 逐 item 對拆後檔案比對；若施工環境可執行腳本，再補算 per-item hash，但 hash 只是比對工具，不取代這個 source-of-truth blob。

允許從 raw body 排除的只有立案白名單：拆檔新增 `use`、必要 module path、最小 visibility plumbing、facade，以及測試 module import plumbing。函式／型別／常數本體不得順手改寫。

## 2. Production top-level item manifest

總數 **62**：`pub` **21**、private **41**。以下 owner 是施工定案切線。

| owner | item | kind | 原 visibility |
|---|---|---|---|
| `card_io.rs` | `PNG_MAGIC` | const | private |
| `card_io.rs` | `string_field` | fn | private |
| `card_io.rs` | `decode_png_character` | fn | private |
| `card_io.rs` | `decode_base64` | fn | private |
| `card_io.rs` | `base64_encode` | fn | private |
| `card_io.rs` | `base64_value` | fn | private |
| `card_io.rs` | `png_chunk` | fn | private |
| `card_io.rs` | `crc32` | fn | private |
| `card.rs` | `PERSONA_FIELDS` | const | private |
| `card.rs` | `PUBLIC_SECTIONS` | const | private |
| `card.rs` | `ImportProbe` | struct | pub |
| `card.rs` | `probe_import` | fn | pub |
| `card.rs` | `import_character` | fn | pub |
| `card.rs` | `card_openings` | fn | pub |
| `card.rs` | `public_markdown` | fn | private |
| `card.rs` | `private_markdown` | fn | private |
| `card.rs` | `worldbook_json` | fn | pub |
| `card.rs` | `persona_as_worldbook` | fn | private |
| `interface.rs` | `InterfaceScript` | struct | pub |
| `interface.rs` | `CardInterface` | struct | pub |
| `interface.rs` | `read_card_interfaces` | fn | pub |
| `interface.rs` | `save_world_card` | fn | pub |
| `interface.rs` | `card_interface` | fn | private |
| `interface.rs` | `is_display_script` | fn | private |
| `interface.rs` | `interface_script` | fn | private |
| `interface.rs` | `is_remote_loader_script` | fn | private |
| `interface.rs` | `is_catch_all_regex` | fn | private |
| `interface.rs` | `card_format_entry` | fn | pub |
| `interface.rs` | `format_tags` | fn | private |
| `mechanism.rs` | `table_tavern_extension` | fn | private |
| `mechanism.rs` | `import_card_extension` | fn | pub |
| `mechanism.rs` | `import_table_tavern_extension` | fn | private |
| `mechanism.rs` | `merge_state_node` | fn | private |
| `mechanism.rs` | `import_mechanism` | fn | pub |
| `mechanism.rs` | `extract_triggers` | fn | private |
| `mechanism.rs` | `entry_marker` | fn | private |
| `mechanism.rs` | `extract_initial_tree` | fn | private |
| `mechanism.rs` | `extract_field_rules` | fn | private |
| `mechanism.rs` | `RuleAttrValues` | type alias | private |
| `mechanism.rs` | `collect_field_rules` | fn | private |
| `mechanism.rs` | `expand_rule_paths` | fn | private |
| `mechanism.rs` | `strip_enclosing_quotes` | fn | private |
| `mechanism.rs` | `expand_segment` | fn | private |
| `mechanism.rs` | `build_field_rule` | fn | private |
| `mechanism.rs` | `insert_initial_tree_node` | fn | private |
| `export.rs` | `export_character` | fn | pub |
| `export.rs` | `character_card_v2` | fn | private |
| `export.rs` | `split_public_markdown` | fn | private |
| `export.rs` | `character_book` | fn | private |
| `export.rs` | `export_base_png` | fn | private |
| `export.rs` | `blank_png` | fn | private |
| `export.rs` | `embed_chara_chunk` | fn | private |
| `images.rs` | `save_gm_image` | fn | pub |
| `images.rs` | `gm_image` | fn | pub |
| `images.rs` | `character_image` | fn | pub |
| `images.rs` | `save_character_image` | fn | pub |
| `images.rs` | `delete_character_image` | fn | pub |
| `images.rs` | `character_avatar` | fn | pub |
| `images.rs` | `save_character_avatar` | fn | pub |
| `images.rs` | `delete_character_avatar` | fn | pub |
| `images.rs` | `save_character_png` | fn | private |
| `images.rs` | `delete_character_png` | fn | private |

## 3. 對外 API / caller baseline

21 個既有 `pub` 全都有 `src-tauri/src/` 外部用途，facade 全數必須保留原 `import::...` 路徑；呼叫端不應因拆檔而修改。

- `commands/character.rs`：`ImportProbe`、`probe_import`、`import_character`、`export_character`
- `commands/world.rs`：`worldbook_json`、`save_world_card`、`save_gm_image`、`import_mechanism`、`import_card_extension`、`card_openings`
- `commands/image.rs`：`character_image`、`save_character_image`、`delete_character_image`、`character_avatar`、`save_character_avatar`、`delete_character_avatar`、`gm_image`
- `commands/refactor.rs`：`CardInterface`、`read_card_interfaces`
- `commands/chat.rs`：`InterfaceScript`、`read_card_interfaces`、`card_format_entry`
- `receipts.rs`：`import_character`

公開 signature 以原 blob 宣告為準；結案要求 21 個原路徑、型別與參數／回傳型別不變。

## 4. DAG 複核與預期 visibility plumbing

施工前複核後，6 檔切線仍是 DAG，沒有 sibling 雙向邊。Production 預期需要的最小 visibility 放寬先鎖成以下 **9 項**；若 compiler 顯示還需要其他項，必須先回寫計畫再加，不能臨場隨便 `pub`：

1. `card_io::PNG_MAGIC` → `pub(super)`：card/interface/images/export 共用。
2. `card_io::string_field` → `pub(super)`：card/interface 共用。
3. `card_io::decode_png_character` → `pub(super)`：card/interface/mechanism 共用。
4. `card_io::base64_encode` → `pub(super)`：images/export 共用，test support 也用。
5. `card_io::png_chunk` → `pub(super)`：export 共用。
6. `card_io::crc32` → `pub(super)`：只為原 `blank_png_is_a_valid_png_container` 測試保持 body 不動。
7. `card::PUBLIC_SECTIONS` → `pub(super)`：export 重建 V2 欄位。
8. `mechanism::table_tavern_extension` → `pub(super)`：export 寫回 Table Tavern extension。
9. `mechanism::import_table_tavern_extension` → `pub(super)`：card import 套用 extension。

`decode_base64` 不預先放寬：原本兩支會直接用它解圖片結果的 image tests 改放到 `card_io.rs` 的 nested tests，讓 test body 原封不動，同時避免為測試擴張 production visibility。`base64_value`、`merge_state_node` 等其餘 implementation detail 維持 private。

## 5. Test leaf baseline

施工前同檔 tests 盤點為 **41 支 `#[test]`**。leaf 名稱如下；結案以 multiset 完全相同為準。

### `card.rs`：12

- `imports_v2_json_and_preserves_original`
- `imports_png_text_chunk_and_preserves_original`
- `imports_v1_top_level_fields`
- `probe_ignores_invalid_bytes`
- `probe_identifies_lorebook_heavy_cards`
- `probe_reports_parsed_state_and_book_entry_count`
- `character_card_brings_its_own_lorebook_entries_to_the_table`
- `imports_alternate_greetings_into_private_markdown`
- `importing_the_same_name_twice_mints_distinct_ids_and_keeps_first_card_intact`
- `card_openings_reads_all_greetings_from_import_bytes`
- `worldbook_json_unwraps_lorebook_cards`
- `worldbook_json_converts_persona_fields_when_card_has_no_entries`

### `mechanism.rs`：7

- `initvar_entry_becomes_initial_tree`
- `initvar_fills_gaps_without_overwriting_existing_values`
- `broken_initvar_never_blocks_import`
- `enabled_initvar_entry_is_ignored`
- `mvu_update_entry_extracts_field_rules_and_marks_incremental_table`
- `ejs_entries_become_triggers_and_log_unrecognized_scripts`
- `wildcard_pipe_segment_expands_into_multiple_rules`

### `export.rs`：6

這裡刻意保留三支跨領域測試，因為它們直接使用 export 私有 PNG helper；放在 export owner 可避免只為測試把 `blank_png`／`embed_chara_chunk` 再開給 sibling。

- `exported_png_reimports_with_identical_content`
- `character_export_import_round_trips_rules_and_initial_tree`
- `exports_freeform_card_as_json`
- `blank_png_is_a_valid_png_container`
- `export_replaces_stale_chara_chunk_in_the_image`
- `save_gm_image_stores_png_and_keeps_it_for_plain_json`

### `card_io.rs`：3

後兩支主要驗 image API，但 body 直接呼叫 private `decode_base64`；放在 card_io 可保持 body 不動且不擴張 visibility。

- `decodes_base64_and_rejects_invalid_input`
- `character_image_returns_png_base64_or_none`
- `saves_and_reads_character_image_and_avatar`

### `images.rs`：2

- `save_character_images_reject_invalid_png_and_missing_character`
- `delete_character_images_is_idempotent`

### `interface.rs`：11

- `read_card_interfaces_filters_to_output_display_scripts`
- `read_card_interfaces_detects_scrypt_cards`
- `read_card_interfaces_detects_remote_loader_cards`
- `read_card_interfaces_handles_plain_cards_without_extensions`
- `read_card_interfaces_skips_characters_with_corrupted_raw_file`
- `save_world_card_persists_display_scripts_for_worldbook_path`
- `save_world_card_skips_plain_worldbook_json`
- `card_interface_fills_opening_from_first_mes`
- `read_card_interfaces_skips_corrupted_world_level_card`
- `card_format_entry_picks_matching_worldbook_entry`
- `card_format_entry_none_when_no_worldbook_entry_matches`

## 6. Test support 定案

`import/test_support.rs` **確定需要**，只收真正跨多個 owner 的共用夾具：

- `TestRoot`（含 `NEXT_TEMP_ID`、`Drop`）
- `minimal_png`

`interface_script_for_test`、`worldbook_entry_for_test` 只服務 interface 測試，留在 `interface.rs` nested tests，不塞進共用 support。

## 7. cfg ledger

拆前 import module 的 cfg 結構只有一個 root：

- `#[cfg(test)] mod tests`（原 `import.rs` 1501 起）

拆後預期改成各 implementation 檔自己的 nested `#[cfg(test)] mod tests`，另 `mod.rs` 只多 `#[cfg(test)] mod test_support;`。不得出現其他 production cfg 漂移。

## 8. 工作段 A 結論

- branch base：**未漂移**。
- source blob：**未漂移**。
- production item：**62 = 21 pub + 41 private**。
- 外部 API：**21 個全數保留 facade；目前沒有零 caller re-export 可拔**。
- test leaf：施工前 inventory **41 支**。
- DAG：原 6 implementation 檔切線成立，未發現 cycle。
- test support：由「可能需要」提升為**確定新增**，內容只限 `TestRoot`＋`minimal_png`。
- 預期 production visibility widen：先鎖 **9 項**，其餘不得預先放寬。

下一工作段從 `card_io.rs` 開始，接著 `images.rs`、`interface.rs`；先做底層與獨立 domain，不碰 mechanism/card/export 的 production body。
