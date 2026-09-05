# data/scene.rs 拆分施工前 baseline

基準日期：2026-09-05  
工作分支：`scene-split`  
基準 `main`：`d6a5be0cb5b835bef1684b060644a1525c4e9b45`  
施工前 branch HEAD：`377d693f8456c0c025a1c0645b167f910daf3436`  
`src-tauri/src/data/scene.rs` blob：`e7ccf217d30977f401afc8f982f24b911f7078f9`

本文件是 `.ai/plans/scene-split.md` 工作段 A 的施工前基準。複核時 `scene-split` 相對 `main` ahead 1 / behind 0，唯一差異是 `.ai/plans/scene-split.md`；`src-tauri/src/data/scene.rs` 與立案時的 immutable blob 完全相同，沒有 source drift。

## 1. Raw body 基準口徑

本輪透過 GitHub connector 直接讀 immutable blob，沒有可執行 repo filesystem；因此**不偽造本地 SHA-256 / per-item hash**。Production 與 tests 的逐 byte source of truth 就是上面的 git blob `e7ccf217…`，搭配下面的 production item 與 test leaf manifest。

施工／結案驗收時，若可執行 `scripts/split-verify/` 或等價腳本，從這個 immutable blob 重新切 item／test body 做機械逐 byte 比對。Hash 是驗證工具，不取代 immutable blob 本身。

允許從 raw body 排除的只有立案白名單：拆檔新增 `use`、必要 module path、下面鎖定的兩項 visibility plumbing、facade 與 test-module import plumbing。函式／型別／常數本體不得順手重寫。

## 2. Production top-level item manifest

總數 **28**：`pub` **17**、`pub(crate)` **4**、private **7**。施工 owner 定案如下。

| owner | item | kind | 原 visibility |
|---|---|---|---|
| `transcript.rs` | `TranscriptKind` | enum | pub |
| `transcript.rs` | `TranscriptEvent` | struct | pub |
| `transcript.rs` | `transcript_path` | fn | private |
| `transcript.rs` | `append_transcript` | fn | pub |
| `transcript.rs` | `append_opening` | fn | pub |
| `transcript.rs` | `rewrite_scene` | fn | private |
| `transcript.rs` | `sync_scene_state_tree` | fn | pub |
| `transcript.rs` | `pop_transcript` | fn | pub |
| `transcript.rs` | `remove_transcript_event` | fn | pub |
| `transcript.rs` | `set_last_transcript_state` | fn | pub |
| `transcript.rs` | `read_transcript` | fn | pub |
| `export.rs` | `render_transcript_entry` | fn | private |
| `export.rs` | `render_scene_section` | fn | private |
| `export.rs` | `export_transcript_markdown` | fn | pub |
| `export.rs` | `export_scene_markdown` | fn | pub |
| `presence.rs` | `CARD_ARRIVAL_PREFIX` | const | pub |
| `presence.rs` | `bracket_title` | fn | pub(crate) |
| `presence.rs` | `appeared_titles` | fn | pub(crate) |
| `presence.rs` | `split_present_names` | fn | pub(crate) |
| `presence.rs` | `name_matches` | fn | pub(crate) |
| `presence.rs` | `settle_card_visibility` | fn | private |
| `lifecycle.rs` | `scene_label` | fn | pub |
| `lifecycle.rs` | `format_scene_summary` | fn | private |
| `lifecycle.rs` | `next_scene_version` | fn | private |
| `lifecycle.rs` | `fork_scene` | fn | pub |
| `lifecycle.rs` | `begin_next_scene` | fn | pub |
| `lifecycle.rs` | `revert_scene` | fn | pub |
| `lifecycle.rs` | `replace_scene_summary` | fn | pub |

## 3. Caller inventory 與 facade 約束

### 3.1 `data/scene/mod.rs` 必須保留的 17 個 `pub`

目前 `data/mod.rs` 已經 `pub use scene::{...}` 下列 17 項，而且完整 caller inventory 複核後**全部都有 scene.rs 外 Rust 用途**；因此新的 `data/scene/mod.rs` 必須全數 re-export，維持既有 `super::scene::...` 與 `data::...` 路徑：

- `TranscriptKind`
- `TranscriptEvent`
- `scene_label`
- `append_transcript`
- `append_opening`
- `sync_scene_state_tree`
- `pop_transcript`
- `remove_transcript_event`
- `set_last_transcript_state`
- `read_transcript`
- `CARD_ARRIVAL_PREFIX`
- `export_transcript_markdown`
- `export_scene_markdown`
- `fork_scene`
- `begin_next_scene`
- `revert_scene`
- `replace_scene_summary`

主要 production caller：

| caller | scene API |
|---|---|
| `data/mod.rs` | 上述 17 個 public API；另見 3.2 的 crate helper |
| `data/world.rs` | `TranscriptEvent`、`TranscriptKind`、`append_transcript` |
| `transport/arrivals.rs` | `TranscriptEvent`、`TranscriptKind`、`CARD_ARRIVAL_PREFIX`、`appeared_titles`、`split_present_names`、`name_matches` |
| `transport/turns.rs` | `TranscriptEvent`、`TranscriptKind` |
| `commands/scene.rs` | `TranscriptEvent`、`scene_label`、`append_transcript`、`append_opening`、`pop_transcript`、`read_transcript`、`CARD_ARRIVAL_PREFIX`、`appeared_titles`、`name_matches`、兩個 export、`fork_scene`、`begin_next_scene`、`revert_scene`、`replace_scene_summary` |
| `commands/chat.rs` | `read_transcript` |
| `commands/state.rs` | `set_last_transcript_state` |
| `refactor/apply.rs` | `sync_scene_state_tree` |
| `genesis.rs` | `append_transcript` |
| `receipts.rs` | `remove_transcript_event` |

另外 `receipts.rs`、`genesis.rs`、`refactor/tests/interface.rs` 等 tests 會再直接用 append/read/pop；這些不改變 production facade 判定，但也是結案 compile/test 會覆蓋到的相容路徑。

### 3.2 `pub(crate)` helper

原 4 項 `pub(crate)` 的外部用途：

- `appeared_titles`：`transport/arrivals.rs`、`commands/scene.rs`，而且 `data/mod.rs` 已 `pub(crate) use`；**必須由新 scene facade re-export**。
- `split_present_names`：`transport/arrivals.rs`，且 `data/mod.rs` 已 `pub(crate) use`；**必須 re-export**。
- `name_matches`：`transport/arrivals.rs`、`commands/scene.rs`，且 `data/mod.rs` 已 `pub(crate) use`；**必須 re-export**。
- `bracket_title`：沒有找到 `data/mod.rs` 之外的直接 consumer，但 `data/mod.rs` 已有 `#[allow(unused_imports)] pub(crate) use scene::bracket_title;`；**scene facade 必須 re-export**。presence 拆分第一次 `cargo test` 以 unresolved import 抓出這個施工前 inventory 漏項；補正後 `data/mod.rs` 仍維持 0 修改，`bracket_title` 定義本身也仍是原 `pub(crate)`，不算 visibility widen。

因此本案目標維持：**`src-tauri/src/data/mod.rs` 0 修改；既有 caller 0 修改。**

## 4. Production dependency DAG 定案

箭頭表示左邊依賴右邊：

```text
export ───────→ transcript
presence ─────→ transcript
lifecycle ────→ transcript
lifecycle ────→ presence
```

實際跨 implementation 邊：

- `export` → `transcript`
  - `TranscriptEvent`
  - `TranscriptKind`
  - `read_transcript`
  - `transcript_path`
- `presence` → `transcript`
  - `TranscriptEvent`
  - `TranscriptKind`
  - `read_transcript`
- `lifecycle` → `transcript`
  - `TranscriptEvent`
  - `TranscriptKind`
  - `append_transcript`
  - `read_transcript`
  - `transcript_path`
- `lifecycle` → `presence`
  - `settle_card_visibility`

`presence` 自己內部的 `appeared_titles`／`split_present_names`／`name_matches` 不形成反向邊；`settle_card_visibility` 雖讀 transcript，但 transcript 不依賴 presence。**零 sibling cycle。**

外部 sibling / crate dependency 維持原樣：

- `transcript` → `data::paths`、`data::state`、crate `mechanism`、`transport::StateBlock`
- `export` → `data::paths`、`data::state`、`local_timestamp`
- `presence` → `data::character`
- `lifecycle` → `data::state`、`local_timestamp`

## 5. Visibility ledger：正式鎖 2 項

施工前複核後，原計畫的兩項候選就是完整最小集合；沒有第三項。

1. `transcript::transcript_path`: private → `pub(super)`
   - `export_scene_markdown` 要判斷單幕檔是否存在。
   - `fork_scene`／`revert_scene`／`replace_scene_summary` 直接讀寫同一路徑。
2. `presence::settle_card_visibility`: private → `pub(super)`
   - `lifecycle::begin_next_scene` 在換幕完成後呼叫一次結算。

其餘 5 個原 private implementation detail **全部保持 private**：

- `transcript::rewrite_scene`
- `export::render_transcript_entry`
- `export::render_scene_section`
- `lifecycle::format_scene_summary`
- `lifecycle::next_scene_version`

若施工時 compiler 顯示還需要第三項 visibility widen，先停、更新 baseline／plan 說明原因，再繼續；不允許直接改成 `pub(crate)` 逃過模組邊界。

## 6. Test leaf baseline

施工前 scene 同檔 tests 共 **25 支 `#[test]`**。依 production owner 的定案落點如下；結案要求 test 名稱 multiset 與函式 body 都跟 immutable blob 一致。

### `transcript.rs`：8

- `transcript_round_trip_is_ordered_jsonl_and_rejects_invalid_kind`
- `pop_transcript_removes_last_event_until_scene_is_empty`
- `append_transcript_uses_current_snapshot_without_overwriting_supplied_state`
- `append_opening_skips_raw_when_nothing_was_stripped`
- `append_opening_merges_state_and_pop_restores_previous_snapshot`
- `pop_transcript_restores_the_previous_event_snapshot`
- `restoring_an_undone_event_puts_its_snapshot_back`
- `pop_transcript_restores_entire_nested_tree_snapshot`

### `export.rs`：4

- `exports_all_transcript_scenes_as_localized_markdown`
- `transcript_export_rejects_a_world_without_scenes`
- `scene_export_contains_only_that_scenes_events`
- `scene_export_rejects_a_missing_scene`

### `lifecycle.rs`：12

- `begin_next_scene_appends_summary_and_advances_scene`
- `begin_next_scene_stores_title_on_old_scene_when_given`
- `revert_scene_returns_to_previous_scene_and_drops_title`
- `revert_scene_rejects_extra_events_without_touching_anything`
- `revert_scene_rejects_at_first_scene`
- `replace_scene_summary_overwrites_text_and_drops_title_when_none`
- `replace_scene_summary_refuses_a_forked_scene`
- `replace_scene_summary_keeps_snapshot_for_later_revert`
- `fork_scene_copies_history_and_relabels_through_continue_and_revert`
- `fork_scene_rejects_current_or_future_scene`
- `fork_scene_rejects_a_scene_with_no_events`
- `scene_label_falls_back_to_original_line_for_unlabeled_scene`

### `presence.rs`：1

- `begin_next_scene_settles_card_auto_hidden`

這支雖透過 `begin_next_scene` 入口觸發，但斷言驗證的是角色卡 `auto_hidden` 換幕結算，因此 owner 定案為 `presence.rs`。

## 7. Test support / cfg 定案

不新增 `data/scene/test_support.rs`。四組 tests 繼續直接使用既有 `crate::data::test_support::*`；目前沒有 scene-only fixture 值得再抽一層共用 support。

拆前 cfg：

- `#[cfg(test)] mod tests` 一個 root。

拆後預期：

- `transcript.rs` / `export.rs` / `presence.rs` / `lifecycle.rs` 各自一個 nested `#[cfg(test)] mod tests`。
- `mod.rs` 不需要 scene 專用 test-support module。
- 不新增任何 production `cfg`。

## 8. 工作段 A 結論

- branch base：**未漂移**；ahead 1 / behind 0，只有立案書。
- source blob：**未漂移**，仍為 `e7ccf217…`。
- production item：**28 = 17 pub + 4 pub(crate) + 7 private**。
- 切線：`transcript` / `export` / `presence` / `lifecycle` 四 implementation 檔＋純 facade，**不調整**。
- DAG：**成立，零 sibling cycle**。
- public facade：17 項全部有外部用途，**全保留**。
- crate facade：`appeared_titles`／`split_present_names`／`name_matches`／`bracket_title` 全數保留；其中 `bracket_title` 是既有 `data/mod.rs` allow-listed re-export，presence 施工首輪由 compiler 補抓此漏項。
- `data/mod.rs`：施工目標 **0 修改**。
- caller：施工目標 **0 修改**。
- production visibility widen：正式鎖定 **2 項**，`transcript_path` 與 `settle_card_visibility`。
- test leaf：**25**，owner 定案 8 / 4 / 12 / 1。
- test support：沿用既有 `data/test_support.rs`，**不新增** scene 專用 fixture 檔。

下一工作段從底層 `transcript.rs` 開始，連同其 8 支 tests 搬家；完成並確認 facade/plumbing 後，再接 `export.rs`。不把四個 implementation 檔一次吞完，避免錯誤來源混在一起。
