# data.rs 拆進 data/

`src-tauri/src/data.rs` 5768 行（本體 2786／同檔測試 2982），是 lib-split-commands 之後全 repo 最大的程式碼檔。目的是維修與管理，不追求運行時效益。2026-08-26 立案，規格經 Sol 兩輪討論定案，無待拍板。

## 切線（本體 2786 行 → 7 檔）

實作階段經 Sol 審核調整兩處，理由都是解除模組環（詳見下方依賴圖）：`SceneLabel` 從 scene 移到 state（它是 `WorldState.scene_labels` 的欄位型別），狀態欄偵測那組從 worldbook 移到 world（它聚合 world／worldbook／character 三個領域）。

`mod.rs` 只當 facade：`mod` 宣告＋`pub use` re-export，外部 `data::CharacterCard`／`data::read_character` 等既有路徑一律不變。真正共用的小型基礎（`DataResult`、`Tier`、`local_timestamp`／`local_timestamp_seconds`、`new_id`、`invalid_data`）留在 `mod.rs`，不為十幾行另造抽象。

| 檔 | 內容（data.rs 行號） |
|---|---|
| `paths.rs` | `validate_id`／`validate_single_line`、`worlds_dir`／`world_dir`／`character_path`／`lanes_path`／`mechanism_log_path`／`import_receipts_path`／`world_card_path`／`gm_image_path`／`interface_shell_path`／`refactor_outcome_path`／`gallery_dir`（590–674） |
| `world.rs` | `WorldMeta`、`last_active`、list／create／delete／rename_world、`create_sample_world`＋`SampleCharacterText`／`SampleWorldText`、`reclaim_world_if_empty`、read／write_world_md、read／write_interface_shell、read／write_refactor_outcome（569–941）、狀態欄偵測 `STATE_BAR_MARKERS`／`declares_state_bar`／`world_has_state_bar`（1305–1347） |
| `worldbook.rs` | `Visibility`／`WorldbookEntry`＋世界書全部行為：JSON 讀寫、存取器、uid／display_index 正規化、read／upsert／reorder／delete、雙向轉換 `worldbook_entry_to_character`／`character_to_worldbook_entry`、`normalize_imported_entry`／`entry_fingerprint`／`is_mechanism_scaffold`、import／dedupe／export（201–226、942–1772） |
| `character.rs` | `CharacterMeta`／`CharacterCard`、frontmatter 解析／serialize、display_index、list／reorder／read／write／delete_character、`read_player_card`、set_character_archived／auto_hidden（127–170、1773–2099） |
| `state.rs` | `SceneLabel`、`TableState`、`StateNode`／`FieldKind`／`UpdateMode`／`InjectLevel`／`FieldRule`、`Condition`／`TriggerMode`／`Trigger`／`TriggerCase`、`Mechanism`（含 `set_tree_value`／`node_at`）、`WorldState`、`read_state`／`write_state`（227–538、2684–2696） |
| `scene.rs` | `TranscriptKind`／`TranscriptEvent`、`scene_label`、append_transcript／append_opening、`rewrite_scene`、`sync_scene_state_tree`、pop／remove／read_transcript、登場前綴工具（`CARD_ARRIVAL_PREFIX`／`bracket_title`／`appeared_titles`／`split_present_names`／`name_matches`）、export_transcript_markdown／export_scene_markdown、fork_scene／begin_next_scene／revert_scene／replace_scene_summary、`settle_card_visibility`（171–200、540–568、2100–2683） |
| `config.rs` | `AppConfig`、read／write_config、read／write_model_catalog、`validate_sponsor_pack`／`sponsor_pack_active`／`install_sponsor_pack`（575–583、2697–2786） |

拆完的 production 依賴是嚴格 DAG，零環：

```
world      依賴 character, paths, scene, state, worldbook   ← 唯一的最上層聚合者
scene      依賴 character, paths, state
worldbook  依賴 character, paths, state
character  依賴 paths, state
state      依賴 paths
paths / config  不依賴任何 sibling
```

**不開 `types.rs`**：把 600 行不同領域的型別塞同一檔只是把胖檔改名，型別跟行為住一起。
**不開 `conversion.rs`**：雙向轉換只有約 120 行，單獨開檔會逼 worldbook 的 raw JSON helper 升可見度，換不到依賴隔離收益。

## 測試

79 支測試按領域同步搬進各檔的 nested `mod tests`，不做「先集中抽 tests.rs」。實測落點：config 6／worldbook 17／world 11／paths 2／character 14／scene 25／state 4。跨領域測試按「驗證的是誰的行為」歸檔，例如 `validate_id_rejects_path_escaping_ids` 走 `write_character` 入口但驗的是 `validate_id`，歸 paths。

集中式 tests 的實際代價比立案時估的小：79 支裡只有 `dedupe_keeps_first_of_each_duplicate_group` 一支直接呼叫私有 fn（`worldbook::write_worldbook_value`），`parse_frontmatter` 之類的私有解析全是透過公開 API 間接驗證。分散配置仍優於集中式——新測試想直接叩私有 fn 時不必再放寬 production 可見度。

共用夾具（`struct TestRoot`＋Drop 清理、`character_card()`、`worldbook_entry()`、`write_worldbook_fixture()`、`read_worldbook_fixture()`）抽成 `data/test_support.rs`，`#[cfg(test)]` 下才存在，夾具標 `pub(super)`。

## 白名單（本案唯一允許的非純搬家動作）

1. 新增 `data/test_support.rs`；
2. 測試專用可見度（夾具 `pub(super)`）與 production 項目最小幅度的 `pub(crate)`／`pub(super)` 調整；
3. module／import plumbing。

其餘 production body 逐 byte 未動，收斂重複、拆函式、改邏輯都不在本案範圍。

## 驗收（2026-08-26 全數通過）

基準存檔：拆前把 data.rs 切成 155 個頂層 item 逐個落檔（切片串接後逐 byte 等於原檔，證明切割無遺漏無重疊），另存 193 條 `pub` 簽名、100 個頂層 `pub` 項目名單與 79 個測試 leaf-name。工具收在 [scripts/split-verify/](../../scripts/split-verify/)。

1. **body 逐 byte**：155 項零遺失零多出，151 項連可見度都完全相同。差異只有兩處，皆為搬檔的必然後果：
   - `sample_world_text` 的 10 行 `include_str!("../samples/*.json")` → `../../samples/`（檔案深了一層）；
   - 下列 3 項升可見度。
2. **對外 `pub` 簽名 multiset**：193 條（含 struct 欄位與 impl 方法）逐字相同、零遺失零改動，after 只多出 3 條 `pub(super)`（即第 5 項）——證明所有 pub 定義都還在且簽名沒被動過。
2b. **facade 完整性**（Sol 審核指出必須分開證明）：拆前 data.rs 的 100 個頂層 `pub`／`pub(crate)` 項目，`mod.rs` 供得出來的正好 100 個，零漏零多、可見度全部一致。上一項的簽名比對刻意跳過 `pub use`，證不到對外路徑；編譯也只覆蓋得到有人呼叫的路徑，沒人叫的 `pub` 項目漏掉 re-export 不會報錯，所以這項要獨立驗。
3. **測試**：`cargo test` 530 全過（與拆前同數字），data:: 79 支 leaf-name multiset 完全相同。
3b. **測試 body 逐 byte**（Sol 指出 leaf-name 證不到斷言有沒有被削弱）：79 支 raw body hash 零遺失零新增零改動。
4. **`RUSTFLAGS="-Dprivate_interfaces" cargo check --all-targets`**：exit 0，零 error 零 warning。
4b. **cfg 分佈**：拆前 8 個 `cfg(unix)`＋1 個 `cfg(not(unix))`，拆後散在 config.rs 4 個、mod.rs 4 個、world.rs 1 個，一一對應零增減。
5. **被迫升可見度的符號**（白名單第 2 項，全為 private → `pub(super)`；這是**目前這條切線下**的最小集合——先不改可見度直接編譯、靠 E0603 列出來才逐一升，但 E0603 只證明得到這個範圍）：

| 符號 | 新家 | 誰要用 |
|---|---|---|
| `worlds_dir` | paths.rs | world.rs 的 `list_worlds`／`reclaim_world_if_empty` |
| `world_dir` | paths.rs | world／worldbook／character／scene／state 五檔都要組路徑 |
| `read_worldbook_value` | worldbook.rs | world.rs 的 `reclaim_world_if_empty` 要判斷世界書是否為空 |

夾具的 `TestRoot`／`TestRoot::new`／`TestRoot::path`／`character_card`／`worldbook_entry`／`write_worldbook_fixture`／`read_worldbook_fixture` 同樣標 `pub(super)`，屬白名單第 2 項的測試專用可見度。

`validate_sponsor_pack`、`SceneLabel`、`refactor_outcome_path`／`validate_id`、`bracket_title` 這 5 個項目在 data 之外無人引用，同檔時不觸發 lint、改成 re-export 才會。它們在 facade 裡按可見度拆成四行單獨 re-export、只有那四行掛 `#[allow(unused_imports)]`，其餘六行維持乾淨（整批掛 allow 會連未來真正多餘的 import 一起遮掉）。拿掉 re-export 則會讓 facade 對外少掉這 5 條路徑。

**已知缺口**：`cargo check --all-targets` 只編 host target，本案沒有 Windows 編譯證據（要等 push 後 CI）；上面的 4b 是本機給得出的最強替代。另外工具比不到 `use` 有沒有綁錯，那條靠人工看 import——data 內無同名型別，各檔 production 頂層的 sibling import 全部明確列名（`mod tests` 內的 `use super::*` 不在此列），已逐檔看過。

不跑全 repo `cargo fmt`（HEAD 本來就不是 fmt-clean）。

## 成果

`data.rs` 5768 行 → `data/` 九檔 5895 行（多出的 127 行是各檔 use 與 nested `mod tests` 的骨架）：

| 檔 | 總行 | 本體 | 測試 |
|---|---:|---:|---:|
| test_support.rs | 78 | — | 78 |
| paths.rs | 145 | 89 | 56 |
| mod.rs | 151 | 151 | — |
| config.rs | 202 | 110 | 92 |
| state.rs | 443 | 347 | 96 |
| world.rs | 666 | 329 | 337 |
| character.rs | 890 | 377 | 513 |
| worldbook.rs | 1544 | 825 | 719 |
| scene.rs | 1776 | 639 | 1137 |
