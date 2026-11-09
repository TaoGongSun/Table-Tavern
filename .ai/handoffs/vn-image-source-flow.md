# vn-image-source-flow

Status: 七項功能與兩項 Grok 必改已實作，待主線工程驗收、原生截圖與真實出圖。分支 `vn-image-source-flow`；只本地 commit，未 push／合併。
規格：[方案與技術細節](../plans/vn-image-source-flow.md)。施工 Sol，工程驗收交主線續 Grok。

## 現況與下一步

- 五個功能包：來源與明確開始、額度暫停恢復、上台補臉、背景與章卡、拒絕與裁邊；前四包 commit 為 `12b6d20d`、`8dee65a0`、`a76c5792`、`90fc9a80`，末包含驗證修正與本交接，見本分支最新 commit。
- 每桌 `vn.json.art` 只存 started／appeared，缺欄預設 false／空；來源仍全域。唯一來源 pause 預檢在 generate_png 的 source_gate 內，暫停／timer不落檔；保留 prepare 與 queue 兩個worker。
- 明確單格／整組重骰由後端直接排指定工作，不受自動 started＋appeared 閘限制；變體先同樣裁邊再比共用，共享仍用定稿畫布／原框。十語系含 vnSpriteTabIntro 已更新。
- 主線下一步：在能監聽 localhost 的環境重跑 `npm run verify`（下方列全受限案例），再安排工程驗收與原生截圖；真實 agy 出圖由主線安排，這輪沒有作為實測通過。
- 不呼叫付費模型、不跑 ignored／錄製夾具、不改 CLI 權限／登入／Connector；本輪未跑 GUI。`.ai/tasks/mobile-room-host.md` 與其他工作線文件未動，不加入本案 commit。

## 驗證與範圍差異

- `npm run verify`：結構、版本、harness隔離、個資、fmt、桌面 Vitest 161檔／2041例、十語系、桌面build、web Vitest 48檔／878通過1跳過、web build、party格式通過；停在party Vitest（273通過、下列5例受TCP限制）。
- 後續步驟補跑：party typecheck／build通過，cargo check通過（既有 CardTexts unused import警告未擴改），harness單元44例通過。最後相關UI63例與後續9例通過，最後tsc與桌面build通過；無原生GUI測試。
- Rust完整首次結果2744通過、89失敗、8 ignored；其中87例都有開本機TCP的PermissionDenied／EPERM。另兩個真錯誤（毫秒浮點取整、新命令分類）已修好，對應測試重跑通過；不把它們列假紅。
- 排除精確87個已確認EPERM案例後的Rust全套補跑：2746通過、0失敗、8 ignored未執行、87 filtered；bin／doc測試亦無失敗。只在命令列指定skip，沒有改測試或提交skip設定。
- 機制維持簡化版，沒有新增持久化待辦／pause、每桌來源、revision、單張探路或合併worker。審查必改已先入方案；來源可用判定、共用畫面狀態與失敗收尾的實際接線使行數超過595估算。
- 計量採本輪基點到收尾的加刪合計，排除i18n、純契約型別、純搬檔；Rust內嵌測試獨立計。核心約 **834** 行，測試 **432** 行，低於核心2倍；i18n差異170行、純前端契約13行另計。行數包含必要註解與格式，沒有用短寫壓行數作驗收。

## 已知限制與未驗證

- 重開後依現有資產重推；不保存強制重生意圖、失敗或pause，首張可能再收到一次額度錯誤。未知reset只手動重試，app關閉不生成。手動候選／變體被暫停後由玩家重試。
- 全域來源切換影響其他桌後續圖；已備好prompt的一張仍可能用舊來源。兩worker無整體先後保證，queue內背景仍先於臉層。
- 拒絕只認已知短句的前綴＋policy片語＋1024bytes上限；其他句型未知，極短故事若恰好符合三條仍可能誤判。串流短暫可見，收尾不存GM成功事件並撤線。
- 舊定稿／臉層不遷移。未驗實際 agy 出圖品質、朋友原圖比例、原生舞台尺寸／滑鼠互動與十語系畫面；這些交主線截圖與真機驗收。

## 受限測試清單

以下只記公開測試名稱，不複製含本機路徑的log。Rust87例均在本機TCP建立點報Operation not permitted；party5例有listen EPERM並逾時。主線應原樣重跑，不提交skip或放寬權限設定。

### party（5例，`party/server/node/node-server.test.ts`）

- 建房、執行端與成員連線；ev 送出當下已落地；關掉用同一 DB 重開後序號接續、舊連線補離線
- 錯誤訊框（超大、非法 UTF-8）與畸形路徑不拖垮程序，別的房照常；靜態檔、穿越、來源、限流
- 45 秒沒心跳的成員被 1001 關掉並廣播離線；送過 ping 的留下
- 玩家卡上傳、執行端取檔、存秘密段走真 HTTP 與 WS；上傳本體與秘密段不寫進 SQLite 檔
- 單頁 fallback 的 index.html 是指向根外的連結時不送

### Rust（87例）

- `commands::image::tests::remote_image_lands_in_gallery_as_png_and_reads_back`
- `generated_image::tests::download_rejects_declared_oversize_body`
- `generated_image::tests::download_rejects_non_success_status`
- `generated_image::tests::download_rejects_undeclared_oversize_body`
- `generated_image::tests::download_sends_no_authorization_header`
- `generated_image::tests::download_times_out`
- `generated_image::tests::download_times_out_when_body_stalls_after_headers`
- `generated_image::tests::remote_webp_is_downloaded_and_converted`
- `openrouter_oauth::tests::key_exchange_times_out_as_network_error`
- `room::member::tests::pipes_frames_both_ways_and_reports_close`
- `room::member::tests::unreachable_server_reports_abnormal_close`
- `room::tests::bad_frame_close_stops_instead_of_looping`
- `room::tests::card_tests::player_cards_follow_seats_and_import_only_the_character`
- `room::tests::closed_room_restores_solo_table`
- `room::tests::create_room_posts_and_keeps_credentials`
- `room::tests::desktop_commands_refuse_the_room_table`
- `room::tests::dispatch_runs_gm_chain_and_resent_dispatch_is_not_landed_twice`
- `room::tests::gallery_tests::crop_save_rechecks_seat_after_cropping`
- `room::tests::gallery_tests::phone_gallery_lists_own_card_pages_and_crops`
- `room::tests::gallery_tests::phone_gallery_page_stays_inside_frame_budget`
- `room::tests::host::host_gm_modes_land_like_the_solo_buttons`
- `room::tests::host::job_undo_pops_the_dispatch_and_rewinds_the_table`
- `room::tests::invite_get_answers_with_stored_secret`
- `room::tests::invite_is_stored_before_ack`
- `room::tests::reconnect_follows_inflight_report`
- `room::tests::remote_control_replies_first_then_acts`
- `room::tests::scene_advance_recomputes_capacity_hint`
- `room::tests::scene_job_reports_progress_and_cancel_answers_by_scene`
- `room::tests::status_reports_capacity_hint_after_gen_done`
- `room::tests::sync::landed_rolls_join_the_comparison`
- `room::tests::sync::reopen_cuts_what_solo_undo_removed_and_bridges_scenes_without_repeating_summaries`
- `room::tests::sync::reopen_sends_solo_play_after_the_common_prefix`
- `room::tests::websocket_goes_through_http_proxy_connect`
- `smart_free::call::tests::live_transport::two_failures_switch_retry_and_every_dispatch_writes_its_own_ledger_line`
- `transport::client::runaway_tests::blank_content_stops_with_runaway_code_not_incomplete`
- `transport::client::runaway_tests::blank_reasoning_trips_unless_policy_is_off`
- `transport::client::runaway_tests::duplicated_reasoning_fields_are_counted_once`
- `transport::client::runaway_tests::length_cap_only_under_full_policy`
- `transport::client::runaway_tests::no_usage_means_nothing_is_recorded`
- `transport::client::runaway_tests::smart_free_stream_reports_runaway_as_emitted_stream_failure`
- `transport::client::runaway_tests::thinking_trip_reports_thinking_count`
- `transport::client::runaway_tests::usage_received_before_runaway_is_recorded_as_is`
- `transport::client::tests::generate_image_keeps_original_error_when_key_lookup_is_inconclusive`
- `transport::client::tests::generate_image_key_lookup_times_out_when_body_stalls`
- `transport::client::tests::generate_image_marks_free_key_on_402_and_403`
- `transport::client::tests::generate_image_rejects_empty_data`
- `transport::client::tests::generate_image_returns_data_url_from_b64_json`
- `transport::client::tests::generate_image_skips_key_lookup_for_other_statuses`
- `transport::client::tests::key_tier_for_is_unknown_when_connection_fails`
- `transport::client::tests::key_tier_for_is_unknown_when_inconclusive`
- `transport::client::tests::key_tier_for_reads_free_and_paid`
- `transport::client::tests::key_tier_for_skips_request_for_blank_key_and_normalizes_saved_base`
- `transport::client::tests::key_tier_for_times_out_when_body_stalls`
- `transport::client::tests::stall_after_usage_still_records_and_reports_timeout`
- `transport::client::tests::stall_keepalive_comments_do_not_extend_deadline`
- `transport::client::tests::stall_role_only_chunk_is_not_progress`
- `transport::client::tests::stall_slow_progressing_stream_survives`
- `transport::client::tests::stall_window_switches_after_first_progress`
- `transport::client::tests::stream_chat_fails_when_stream_completes_with_no_content`
- `transport::client::tests::stream_chat_passes_usage_chunk_through_without_breaking_deltas`
- `transport::client::tests::stream_chat_streams_deltas_from_mock_server_and_requires_key_for_openrouter`
- `transport::client::timeout_tests::error_body_stall_keeps_status`
- `transport::client::timeout_tests::generate_image_headers_never_arrive_times_out`
- `transport::client::timeout_tests::generate_image_success_body_stall_times_out`
- `transport::client::timeout_tests::headers_never_arrive_reports_stalled`
- `transport::client::timeout_tests::late_headers_plus_late_progress_share_one_deadline`
- `transport::client::timeout_tests::progressing_stream_outlives_first_window`
- `transport::client::timeout_tests::smart_error_body_stall_keeps_status_and_headers`
- `transport::client::timeout_tests::smart_headers_never_arrive_is_timeout`
- `transport::responses::tests::responses_blank_text_and_reasoning_stop_with_runaway_code`
- `transport::responses::tests::responses_error_body_stall_keeps_status`
- `transport::responses::tests::responses_headers_never_arrive_reports_stalled`
- `transport::responses::tests::responses_reasoning_counts_as_progress_and_slow_stream_survives`
- `transport::responses::tests::responses_stall_after_created_only_and_keepalive`
- `transport::responses::tests::responses_window_switches_after_first_progress`
- `updater::net::tests::check_times_out_on_trickling_manifest`
- `updater::net::tests::check_times_out_when_head_never_comes`
- `updater::net::tests::download_is_not_capped_by_check_total`
- `updater::net::tests::download_times_out_when_body_stalls`
- `updater::net::tests::fetch_follows_redirect`
- `updater::net::tests::fetch_keeps_slow_steady_body`
- `updater::net::tests::fetch_stops_after_five_redirects`
- `updater::net::tests::fetch_times_out_when_body_stalls`
- `updater::net::tests::fetch_times_out_when_head_never_comes`
- `updater::net::tests::https_only_rejects_redirect_to_http`
- `updater::net::tests::production_fetch_refuses_http`
- `updater::net::tests::redirect_to_http_works_without_https_only`

## 本案檔案前後行數

基準為本輪開工時的本分支狀態；新增檔為0，搬檔列原位置→新位置。未列／未改其他工作線。

| 檔案 | 前 | 後 |
| --- | ---: | ---: |
| `.ai/handoffs/vn-image-source-flow.md` | 13 | 188 |
| `.ai/plans/vn-image-source-flow.md` | 133 | 145 |
| `src-tauri/src/cli/mod.rs` | 52 | 53 |
| `src-tauri/src/cli/runner.rs` | 905 | 914 |
| `src-tauri/src/cli/runner/finish_tests.rs` | 303 | 342 |
| `src-tauri/src/cli/stream.rs` | 599 | 613 |
| `src-tauri/src/cli/stream/agy_errors.rs` | 0 | 186 |
| `src-tauri/src/commands/vn_sprites.rs` | 355 | 482 |
| `src-tauri/src/data/vn.rs` | 856 | 870 |
| `src-tauri/src/data/vn_tests.rs` | 214 | 229 |
| `src-tauri/src/generated_image/cli_fetch.rs` | 342 | 353 |
| `src-tauri/src/lanes/mod.rs` | 1022 | 1029 |
| `src-tauri/src/lanes/tests/agy_cleanup.rs` | 331 | 388 |
| `src-tauri/src/lib.rs` | 630 | 636 |
| `src-tauri/src/transport/dispatch.rs` | 916 | 917 |
| `src-tauri/src/transport/vn_sprite_prompts.rs` | 129 | 129 |
| `src-tauri/src/vn_backgrounds.rs` | 645 | 654 |
| `src-tauri/src/vn_sprites.rs` | 613 | 629 |
| `src-tauri/src/vn_sprites/pause.rs` | 0 | 107 |
| `src-tauri/src/vn_sprites/prepare.rs` | 279 | 322 |
| `src-tauri/src/vn_sprites/queue.rs` | 489 | 585 |
| `src/features/vn/ChapterCardView.tsx` | 67 | 71 |
| `src/features/vn/SpriteBasePicker.tsx` | 129 | 97 |
| `src/features/vn/Stage.tsx` | 144 | 148 |
| `src/features/vn/StageBackdrop.tsx` | 116 | 125 |
| `src/features/vn/TitleScreen.tsx` | 111 | 99 |
| `src/features/vn/VnPlayer.tsx` | 750 | 754 |
| `src/features/vn/art-flow/ArtFlowNotice.tsx` | 0 | 46 |
| `src/features/vn/art-flow/SpriteSourceSelect.tsx` | 0 | 34 |
| `src/features/vn/wizard/useArtProgress.test.ts → src/features/vn/art-flow/useArtProgress.test.ts` | 63 | 72 |
| `src/features/vn/wizard/useArtProgress.ts → src/features/vn/art-flow/useArtProgress.ts` | 62 | 62 |
| `src/features/vn/background-flow.test.tsx` | 0 | 41 |
| `src/features/vn/gallery/ArtGallery.test.tsx` | 238 | 238 |
| `src/features/vn/gallery/ArtGallery.tsx` | 65 | 67 |
| `src/features/vn/gallery/BackgroundGallery.tsx` | 213 | 213 |
| `src/features/vn/useBackgrounds.ts` | 86 | 95 |
| `src/features/vn/useSpriteSlot.ts` | 134 | 139 |
| `src/features/vn/useSprites.test.ts` | 0 | 29 |
| `src/features/vn/useSprites.ts` | 47 | 50 |
| `src/features/vn/vn-player.css` | 518 | 521 |
| `src/features/vn/wizard/ArtStep.tsx` | 169 | 168 |
| `src/features/vn/wizard/IdeaStep.tsx` | 194 | 197 |
| `src/features/vn/wizard/VnWizard.test.tsx` | 259 | 264 |
| `src/features/vn/wizard/VnWizard.tsx` | 360 | 363 |
| `src/i18n/features/vn-mode.ts` | 1007 | 1007 |
| `src/i18n/features/vn-wizard.ts` | 787 | 937 |
| `src/shared/contracts/backend-contracts.ts` | 540 | 553 |
| `src/shared/ui/ai-error.ts` | 141 | 142 |
| `src/shared/ui/atoms.tsx` | 89 | 90 |
| `src-tauri/src/vn_sprites/trim.rs` | 0 | 79 |
| `src/features/vn/art-flow/ArtFlowNotice.test.tsx` | 0 | 31 |
