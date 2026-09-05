# cli.rs 拆分施工前 baseline

基準日期：2026-09-05  
工作分支：`cli-split`  
基準 `main`：`8dfecfa20b9c3d026752a76c9848312f29c7d5fe`  
施工前 branch HEAD：`7cd65cae5611bdddc97e08ca91f98e94df9415fe`  
`src-tauri/src/cli.rs` blob：`2fb6f4ab9808e5b64a1c1c4fd9aaab2ac0e4a212`

本文件是 `.ai/plans/cli-split.md` 工作段 A 的施工前基準。開工複核時 `main` 仍停在立案基準，工作分支相對 `main` 只有立案書 commit，因此原始 `cli.rs` blob 未漂移。

## 1. Raw body 基準口徑

這輪透過 GitHub connector 直接讀 immutable blob，沒有可執行 repo filesystem；因此沿用 `import-split` 已採用的基準口徑：**不偽造本地 SHA-256**。Production 與 tests 的逐 byte 原文基準就是上面的 immutable git blob `2fb6f4ab…`，搭配下面的 item／test leaf manifest。

原檔共 2087 行：production 1–1176、空行 1177、`#[cfg(test)] mod tests` 1178–2087。結案 integrity 驗收必須直接從這個 immutable blob 取原文，逐 item／逐 test body 對拆後檔案；若施工環境可執行既有 `scripts/split-verify/`，再由工具產生 slice hash／byte range。hash 是比對工具，不取代這個 source-of-truth blob。

允許排除的差異只有立案白名單：拆檔新增 `use`、必要 module path、最小 visibility plumbing、facade，以及測試 module import plumbing。函式／型別／常數本體不得順手改寫。

## 2. Production top-level item manifest

總數 **45**：`pub` **33**、`pub(crate)` **1**、private **10**、`impl` **1**。以下 owner 是施工定案切線。

| owner | item | kind | 原 visibility |
|---|---|---|---|
| `types.rs` | `CliInfo` | struct | pub |
| `types.rs` | `ModelOption` | struct | pub |
| `types.rs` | `CliSession` | enum | pub |
| `types.rs` | `CliLine` | enum | pub |
| `types.rs` | `UsageLog` | struct | pub |
| `detect.rs` | `candidate_dirs` | fn | private |
| `detect.rs` | `is_executable` | fn | private |
| `detect.rs` | `find_binary` | fn | pub(crate) |
| `detect.rs` | `hidden_output` | async fn | private |
| `detect.rs` | `probe_cli` | async fn | private |
| `detect.rs` | `detect_clis` | async fn | pub |
| `catalog.rs` | `parse_codex_catalog` | fn | pub |
| `catalog.rs` | `parse_claude_registry` | fn | pub |
| `catalog.rs` | `parse_agy_catalog` | fn | pub |
| `catalog.rs` | `parse_grok_catalog` | fn | pub |
| `catalog.rs` | `cli_model_catalog` | async fn | pub |
| `request.rs` | `flatten_messages` | fn | pub |
| `request.rs` | `tier_override` | fn | pub |
| `request.rs` | `claude_model_for` | fn | pub |
| `request.rs` | `codex_effort_for` | fn | pub |
| `request.rs` | `claude_args` | fn | pub |
| `request.rs` | `claude_session_args` | fn | pub |
| `request.rs` | `codex_args` | fn | pub |
| `request.rs` | `agy_supports_stream_json` | fn | pub |
| `request.rs` | `agy_args` | fn | pub |
| `request.rs` | `grok_envs` | fn | pub |
| `request.rs` | `GROK_SAMPLING_OVERLAY` | const | pub |
| `request.rs` | `GROK_CHAT_DISALLOWED_TOOLS` | const | private |
| `request.rs` | `grok_args` | fn | pub |
| `request.rs` | `grok_session_args` | fn | pub |
| `request.rs` | `grok_common_args` | fn | private |
| `stream.rs` | `parse_claude_line` | fn | pub |
| `stream.rs` | `usage_event` | fn | private |
| `stream.rs` | `token_count` | fn | private |
| `stream.rs` | `parse_claude_usage` | fn | pub |
| `stream.rs` | `parse_codex_usage` | fn | pub |
| `stream.rs` | `parse_grok_usage` | fn | pub |
| `stream.rs` | `parse_agy_usage` | fn | pub |
| `stream.rs` | `parse_codex_line` | fn | pub |
| `stream.rs` | `parse_agy_line` | fn | pub |
| `stream.rs` | `parse_grok_line` | fn | pub |
| `runner.rs` | `ChildPidGuard` | tuple struct | private |
| `runner.rs` | `Drop for ChildPidGuard` | impl | n/a |
| `runner.rs` | `api_error_kind` | fn | private |
| `runner.rs` | `run_cli` | async fn | pub |

## 3. Caller inventory 與 facade baseline

repo-wide caller 複核後，立案書的「至少包括」名單需要補兩個實際 production caller：`commands/settings.rs` 與 `ai_transport.rs`。目前需要維持舊 `crate::cli::...` 路徑的 Rust caller 範圍為：

- `commands/settings.rs`：`CliInfo`、`ModelOption`、`detect_clis`、`cli_model_catalog`、`tier_override`、`claude_model_for`
- `data/config.rs`：`ModelOption`
- `ai_transport.rs`：`grok_envs`、`detect_clis`、`tier_override`、`claude_model_for`、`agy_supports_stream_json`、`flatten_messages`、`claude_args`、`codex_args`、`codex_effort_for`、`agy_args`、`grok_args`、`parse_claude_line`、`parse_codex_line`、`parse_agy_line`、`parse_grok_line`、`parse_claude_usage`、`parse_codex_usage`、`parse_agy_usage`、`parse_grok_usage`、`UsageLog`、`run_cli`
- `transport/client.rs`：`tier_override`、`claude_model_for`、`codex_effort_for`
- `lanes.rs`：`CliSession`、Claude／Grok session args、stream/usage parser、`UsageLog`、`run_cli`
- `refactor_session.rs`：`CliSession`、Claude session args、Claude stream/usage parser、`UsageLog`、`run_cli`
- `inflight.rs`：`run_cli`、`parse_claude_line`
- `transport/assemble.rs`：`flatten_messages`
- `commands/cli_setup.rs`：`find_binary`（production）；另有 `grok_envs` test caller

### facade 必須保留：28 項

其中 `find_binary` 保持 `pub(crate)` facade，其餘依原 visibility 保持 `pub`：

- types：`CliInfo`、`ModelOption`、`CliSession`、`UsageLog`
- detect：`detect_clis`、`find_binary`
- catalog：`cli_model_catalog`
- request：`flatten_messages`、`tier_override`、`claude_model_for`、`codex_effort_for`、`claude_args`、`claude_session_args`、`codex_args`、`agy_supports_stream_json`、`agy_args`、`grok_envs`、`grok_args`、`grok_session_args`
- stream：`parse_claude_line`、`parse_claude_usage`、`parse_codex_usage`、`parse_grok_usage`、`parse_agy_usage`、`parse_codex_line`、`parse_agy_line`、`parse_grok_line`
- runner：`run_cli`

### 零外部 caller，不掛 facade：6 項

以下雖原本宣告為 `pub`，repo-wide 搜尋只有 `cli.rs` 自身／同檔 tests（或文件紀錄），拆後**不**由 `mod.rs` re-export：

- `parse_codex_catalog`
- `parse_claude_registry`
- `parse_agy_catalog`
- `parse_grok_catalog`
- `GROK_SAMPLING_OVERLAY`
- `CliLine`

這不是改變 item 本身原 visibility；implementation 檔內仍保持原 `pub`，只是 private child module 不把它重新掛回 `crate::cli` facade。

## 4. Production DAG 定案

箭頭表示「左邊依賴右邊」：

```text
catalog ─────→ detect ─────→ types
   │                         ↑
   └─────────────────────────┘

request ───────────────────→ types
stream  ───────────────────→ types
runner  ───────────────────→ types
```

外部 crate module 依賴另計：

- `types.rs` → `transport::PromptCacheUsage`、`usage_log::{LaneContext, PromptShape}`
- `request.rs` → `data::Tier`、`transport::ChatMessage`
- `catalog.rs` → serde_json、regex、tokio blocking task
- `runner.rs` → `data::DataResult`、`proxy`、`inflight`、`transport::describe`、`usage_log`、tokio process/io/time

複核結果沒有 sibling cycle，也沒有理由改成 provider 垂直切；原六 implementation 檔切線正式成立。

## 5. Visibility ledger

施工前鎖定：production **只有 1 項**需要因 sibling 呼叫由 private 最小放寬：

1. `detect::hidden_output` → `pub(super)`：`catalog::cli_model_catalog` 的 agy／grok `models` 路徑需要呼叫。

其餘：

- `detect::find_binary` **維持原 `pub(crate)`**，不能收窄成 `pub(super)`：`commands/cli_setup.rs` 有 production caller；catalog 也會使用。
- `request::grok_common_args` 維持 private。
- `request::GROK_CHAT_DISALLOWED_TOOLS` 維持 private。
- `stream::usage_event`、`stream::token_count` 維持 private。
- `runner::ChildPidGuard`、`runner::api_error_kind` 維持 private。
- `CliLine`、`UsageLog` 等跨 sibling 型別原本已是 `pub`，不構成 visibility widen；只是 `CliLine` 不 re-export 到 facade。

若 compiler 顯示還需要第 2 項 visibility widen，必須先停下來回寫 ledger，不能臨場加 `pub`。

## 6. Test leaf baseline

施工前 tests 共 **30 支 leaf**（`#[test]`／`#[tokio::test]`）。結案以名稱 multiset 與 function body 原文完全相同為準。

### `request.rs`：10

- `flatten_restores_speaker_prefix_and_appends_turn_instruction`
- `agy_stream_json_support_gates_on_1_1_8`
- `flatten_skips_label_and_closing_when_self_contained`
- `agy_args_put_prompt_in_final_p_value_with_optional_model`
- `grok_args_disable_every_tool_and_put_prompt_last`
- `grok_session_args_open_carries_system_and_resume_does_not`
- `grok_envs_point_home_and_grok_home_at_the_app_profile`
- `tier_mappings_cover_all_tiers`
- `claude_session_args_keep_persistence_and_pick_session_flag`
- `tier_override_reads_prefixed_keys_and_ignores_blank`

### `catalog.rs`：5

- `codex_catalog_skips_internal_and_hidden_and_sorts_by_priority`
- `claude_registry_lists_newest_first_and_drops_legacy`
- `claude_registry_catches_entry_across_chunk_boundary`
- `agy_catalog_takes_id_column_only`
- `grok_catalog_ignores_noise_and_strips_default_marker`

### `stream.rs`：10

- `parses_real_claude_stream_json_lines`
- `parses_claude_usage_adding_cache_tokens_to_input`
- `parses_codex_usage_without_double_counting_cached_input`
- `parses_grok_usage_adding_cache_read_to_input`
- `missing_usage_fields_count_as_zero`
- `parses_real_codex_json_lines_and_ignores_warning_items`
- `parses_agy_stream_json_events`
- `parses_agy_usage_from_result_event`
- `agy_usage_picks_contract_by_total_and_bails_out_when_neither_fits`
- `parses_grok_streaming_json_lines`

### `runner.rs`：5

- `run_cli_streams_deltas_from_fake_cli_and_reads_stdin`
- `run_cli_aborts_instantly_on_fatal_stderr_api_error_and_shows_it_in_tail`
- `run_cli_reports_crash_without_result_event_instead_of_returning_partial_text`
- `run_cli_strips_inherited_anthropic_env_but_keeps_explicit_envs`
- `api_error_kind_classifies_fatal_vs_transient`

`detect.rs`／`types.rs` 沒有 owner test leaf。

## 7. Test support 定案

**不新增 `test_support.rs`。**

原 tests 只有兩個 helper：

- `msg` 只服務 `flatten_messages` 測試，跟進 `request.rs` nested tests。
- `registry_entry` 只服務 Claude registry 測試，跟進 `catalog.rs` nested tests。

沒有跨 owner 共用 fixture，為了形式硬抽 `test_support.rs` 只會增加 plumbing。

## 8. cfg ledger

拆前 cli module 的 cfg root：

- `#[cfg(test)] mod tests`（1178–2087）
- production 內既有平台 cfg（Unix executable 檢查、Windows candidate path／`creation_flags`）必須跟原 owner body 一起搬，不得改條件。

拆後只把 root test module 分散成各 implementation 檔自己的 nested `#[cfg(test)] mod tests`；`mod.rs` 不需要 `test_support` 宣告。不得出現其他 cfg 語意漂移。

## 9. 工作段 A 結論

- branch base：**未漂移**；`main` 仍是 `8dfecfa2…`。
- source blob：**未漂移**；仍是 `2fb6f4ab…`。
- production item：**45 = 33 pub + 1 pub(crate) + 10 private + 1 impl**。
- 外部 facade：**28 項保留**；其中 `find_binary` 為 `pub(crate)`。
- 零 caller public item：**6 項不掛 facade**。
- test leaf：**30 支**，owner = request 10 / catalog 5 / stream 10 / runner 5。
- DAG：六 implementation 檔切線成立，沒有 sibling cycle。
- test support：**不需要**。
- 預期 production visibility widen：**只有 `hidden_output → pub(super)` 1 項**。

下一工作段 B 可以直接開始 `types.rs` → `detect.rs` → `catalog.rs`，先搬 leaf／低層依賴；不得碰 request/stream/runner 的 production body。