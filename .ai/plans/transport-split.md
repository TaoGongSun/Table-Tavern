# transport.rs 拆進 transport/

`src-tauri/src/transport.rs` 原為 5472 行（本體 2198／同檔測試 3274）。2026-08-26 立案，規格經 Sol 兩輪討論；2026-09-01 開工前再核對現況後補強切線、完整依賴圖與可見度規則，並於同日完成實作。正式程式 commit：`8f26fb71f1eeb99ed6d9ffc83de9fe3a4cd20aec`（`refactor: split transport module`）。做法與驗收沿用 [data-split](data-split.md)，本檔記錄本案切線與最後結果。

## 切線（本體 2198 行 → 8 檔）

原本以為的「組裝（1–1230）／解析（1269–1700）／傳輸（1709–2198）」三段是假的：實測 `gm_system_prompt` 被行 1192 呼叫、`gm_dynamic_block` 被 1216 呼叫、`active_worldbook_entries` 被 1108／1212，前 528 行反而是下游的被呼叫端。

開工前複核再調一處：`gm_dynamic_block` 跟 `TreeRender`／`render_state_tree` 放在 `state_view.rs`。它直接組 `TreeRender` 並呼叫 `render_state_tree`；若留在 `context.rs`，拆檔後會被迫把這兩個純 state-render implementation detail（連同 `TreeRender` 欄位）開給 sibling。搬到 `state_view.rs` 可讓兩者維持 private，production body 不需改字。

`transport/mod.rs` 只當 facade：`mod` 宣告＋`pub use`／`pub(crate) use` re-export，外部既有 `transport::ChatMessage`、`transport::stream_chat`、`transport::extract_state_block` 等路徑一律不變。必要的 sibling 可見度只做最小放寬，不把內部 helper 無故升成 crate API。

完整 production 依賴如下；箭頭 `A → B` 表示 **B 依賴 A**：

```
messages ─┬→ state_view ─┐
          ├→ context ────┼→ turns ─┐
          ├→ arrivals    │         │
          ├→ client      └─────────┼→ assemble ─→ response
          └────────────────────────┘            ↑
          └─────────────────────────────────────┘
```

展開成直接依賴：

- `state_view` → `messages`
- `context` → `messages`
- `turns` → `messages`、`state_view`、`context`
- `assemble` → `messages`、`state_view`、`context`、`turns`
- `arrivals` → `messages`
- `response` → `messages`、`assemble`（`PLAYER_SENTINEL`）
- `client` → `messages`（`ChatMessage`）

上述為 DAG，無 sibling module 環。

| 檔 | 內容（原 transport.rs 行號） |
|---|---|
| `messages.rs` | `ChatMessage`、`message`、`push_merged`、`language_rule`、`player_fallback_name`、`resolve_display_macros`、`replace_st_macros`（20–119） |
| `state_view.rs` | `gm_dynamic_block`、`TreeRender`／`render_state_tree`、`trigger_scope_hidden`、`StateScope`／`state_scope`、`resolve_branch`／`auto_match_branch`、`character_state_block`、`snapshot_updates`／`collect_snapshot_updates`（529–704、839–974；另含原 449–528 的 `gm_dynamic_block`） |
| `context.rs` | `active_worldbook_entries`、`gm_system_prompt`、`mechanism_protocol`、`interface_owned_notice`、`split_person_roster`（120–161、273–448、705–722） |
| `assemble.rs` | `PLAYER_SENTINEL`、`assemble_shared_messages`、`assemble_gm_messages`（162–272） |
| `turns.rs` | `LaneTurn`、`system_event_text`、`lane_event_line`、chars_lane_system／turn、gm_lane_system／turn、`summary_messages`（975–1268） |
| `arrivals.rs` | `PERSON_ARRIVAL_PREFIX`、`arrival_title`、`appeared_person_titles`／`appeared_card_names`、`detect_new_arrivals`／`detect_new_card_arrivals`、`person_arrival_text`／`card_arrival_text`（723–838） |
| `response.rs` | `extract_scene_title`、`narrate_instruction`、`card_format_instruction`、`extract_next_speaker`、`StateTag`／`find_state_tag`、`StateBlock`／`extract_state_block`、`parse_indented_fields`、`pick_speaker`（1269–1708） |
| `client.rs` | `DEFAULT_BASE_URL`／`DEFAULT_IMAGE_MODEL`、ui_language／gm_tier／refactor_expand_tier／resolve_model／`TierModel`／tier_model／base_url、`SseParser`、`PromptCacheUsage`／`extract_usage`／`cache_tokens`／`describe`、`anthropic_messages`／`chat_request_body`／`extract_delta`、`StreamOutcome`、`http_error`、`stream_chat`、`generate_image`（13–19、1709–2198） |

`assemble_shared_messages` 呼叫 `chars_lane_turn`，故 assemble 位於 turns 上游。命名叫 `turns.rs` 不叫 `lanes.rs`——crate 根已有 1567 行的 `lanes.rs`（lane 續聊狀態機），撞名會誤導。

## 測試

實作時重新盤點確認 transport 共有 **91 支測試函式**：86 支同步 `#[test]` 加 5 支 `#[tokio::test] async fn`。全部按領域同步搬進各檔 nested `mod tests`；共用夾具 `card()`、`event()`、`worldbook_entry()` 抽成 `transport/test_support.rs`。舊規格寫的「86 支」只數到同步測試，已作廢。

## 白名單（本案唯一允許的非純搬家動作）

1. 新增 `transport/test_support.rs`；
2. 測試專用可見度，以及因 sibling module 分拆被編譯器迫使產生的 production **最小** `pub(super)`／`pub(crate)` 調整；
3. `transport/mod.rs` facade、module 宣告、re-export 與 import plumbing；
4. 搬檔造成的相對路徑調整（若實際存在）。

其餘 production body 逐 byte 不動；不趁本案收斂重複、拆函式、改命名或改邏輯。

最後實際需要的 production 可見度放寬共 **7 項**，全部只到 `pub(super)`：`gm_dynamic_block`、`gm_system_prompt`、`language_rule`、`message`、`push_merged`、`split_person_roster`、`system_event_text`。沒有其他 production item 內容變更。

## 驗收結果

2026-09-01 完整沿用 [data-split](data-split.md) 的基準抓取與驗收強度，結果全綠：

1. production 頂層 item：拆前 76／拆後 76，遺失 0、多出 0；69/76 含可見度逐 byte 相同，另外 7 項只有上述 `pub(super)` 可見度差異，**內容變更 0**；
2. 對外 `pub`／`pub(crate)` 簽名 baseline 全保留，只多 7 個預期的 sibling `pub(super)`；
3. `transport/mod.rs` facade：拆前 top-level 對外項目 52、拆後提供 52，遺失 0、多出 0、對外可見度變更 0；
4. `cargo test` 全綠：全庫 527 個 test leaf 名稱 multiset 拆前拆後完全相同；實際執行 523 passed、0 failed、4 ignored；
5. transport **91 支**測試函式 raw body hash 全部 byte-identical，遺失 0、新增 0、內容變更 0；
6. `RUSTFLAGS="-Dprivate_interfaces" cargo check --all-targets` 全綠；
7. cfg ledger 為 8 個 `#[cfg(test)]`，無非預期 cfg 漂移；
8. 前端 `npm run build` 全綠；既有 warning 不屬於本案失敗；
9. 正式 main commit 只包含原 `transport.rs` 移除與 `transport/` 正式模組檔，驗收用暫時 workflow／script 沒有帶入 main。

## 結果

**完成。** `src-tauri/src/transport.rs` 已拆成 `src-tauri/src/transport/`，正式程式 commit 為 `8f26fb71f1eeb99ed6d9ffc83de9fe3a4cd20aec`。本案未與其他 refactor 混在同一個程式 commit；後續若要再重構 transport 內部邏輯，應另立新案，不回頭把本次純搬家擴 scope。
