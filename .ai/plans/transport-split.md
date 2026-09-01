# transport.rs 拆進 transport/

`src-tauri/src/transport.rs` 5472 行（本體 2198／同檔測試 3274）。2026-08-26 立案，規格經 Sol 兩輪討論定案，無待拍板。做法與驗收沿用 [data-split](data-split.md)，本檔只記與它不同的部分。

## 切線（本體 2198 行 → 8 檔）

原本以為的「組裝（1–1230）／解析（1269–1700）／傳輸（1709–2198）」三段是假的：實測 `gm_system_prompt` 被行 1192 呼叫、`gm_dynamic_block` 被 1216 呼叫、`active_worldbook_entries` 被 1108／1212，前 528 行反而是下游的被呼叫端。按實際依賴方向重排後無循環：

```
messages → state_view → context → turns → assemble
arrivals → messages
```

| 檔 | 內容（transport.rs 行號） |
|---|---|
| `messages.rs` | `ChatMessage`、`message`、`push_merged`、`language_rule`、`player_fallback_name`、`resolve_display_macros`、`replace_st_macros`（20–119） |
| `state_view.rs` | `TreeRender`／`render_state_tree`、`trigger_scope_hidden`、`StateScope`／`state_scope`、`resolve_branch`／`auto_match_branch`、`character_state_block`、`snapshot_updates`／`collect_snapshot_updates`（529–704、839–974） |
| `context.rs` | `active_worldbook_entries`、`gm_system_prompt`、`mechanism_protocol`、`interface_owned_notice`、`gm_dynamic_block`、`split_person_roster`（120–161、273–528、705–722） |
| `assemble.rs` | `PLAYER_SENTINEL`、`assemble_shared_messages`、`assemble_gm_messages`（162–272） |
| `turns.rs` | `LaneTurn`、`system_event_text`、`lane_event_line`、chars_lane_system／turn、gm_lane_system／turn、`summary_messages`（975–1268） |
| `arrivals.rs` | `PERSON_ARRIVAL_PREFIX`、`arrival_title`、`appeared_person_titles`／`appeared_card_names`、`detect_new_arrivals`／`detect_new_card_arrivals`、`person_arrival_text`／`card_arrival_text`（723–838） |
| `response.rs` | `extract_scene_title`、`narrate_instruction`、`card_format_instruction`、`extract_next_speaker`、`StateTag`／`find_state_tag`、`StateBlock`／`extract_state_block`、`parse_indented_fields`、`pick_speaker`（1269–1708） |
| `client.rs` | `DEFAULT_BASE_URL`／`DEFAULT_IMAGE_MODEL`、ui_language／gm_tier／refactor_expand_tier／resolve_model／`TierModel`／tier_model／base_url、`SseParser`、`PromptCacheUsage`／`extract_usage`／`cache_tokens`／`describe`、`anthropic_messages`／`chat_request_body`／`extract_delta`、`StreamOutcome`、`http_error`（13–19、1709–2198） |

`assemble_shared_messages` 呼叫 `chars_lane_turn`，故 assemble 位於 turns 上游。命名叫 `turns.rs` 不叫 `lanes.rs`——crate 根已有 1567 行的 `lanes.rs`（lane 續聊狀態機），撞名會誤導。

## 測試

86 支測試按領域同步搬進各檔 nested `mod tests`；共用夾具 `card()`、`event()`、`worldbook_entry()` 抽成 `transport/test_support.rs`。

## 順序

data-split 完成並 commit 後才動本案，不與它綁同一波。
