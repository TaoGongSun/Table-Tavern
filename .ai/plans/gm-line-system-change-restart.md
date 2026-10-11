# gm-line-system-change-restart 方案

## 一、裁決
- GM 線凍結 system 因動態巨集（world.md、角色卡等）變動時整線重開，比照 `Hoist::All`／Grok，Claude 不再在 session 裡疊補丁〔作者裁決 2026-10-11〕。
- 尾段疊加：回合後抹掉尾段世界書〔作者裁決 2026-10-11〕，由待辦 tail-worldbook-erase 處理，本案不動尾段。

## 二、現況（掛點）
- 組裝：`gm_narrate` → `commands/chat.rs` `gm_lane_reply`（唯一呼叫點）→ `chat_assembly::gm_lane_parts` → `transport::gm_lane_system`（GM 指示＋world.md＋穩定前／後組條目＋全卡＋玩家卡＋機制協定）與 `gm_lane_turn`（不穩定條目、作者註記、依深度＋目前狀態＋導演指示）。
- 比對：`lanes/mod.rs` `plan_turn`。
  - Grok／Agy：`state.applied != frozen_system` → `Reopen(SystemChanged)`。
  - Claude（GM 與角色線共用）：快取內送舊 `snapshot`＋`snapshot_patch::render_patch` 補丁接在本輪 tail 前；過期則同 session 換上新 system（`rebased`）。補丁回合後不抹，留在 session 歷史。world.md／卡含動態巨集時每輪一份。
- 量測：`scene_budget/measure.rs` `gm_request` 走同一組裝（`gm_lane_parts`），以 `Randomness::Measure` 與唯讀巨集量「重開全量」一份；`path_budget` 取它與 `lanes::last_prompt_tokens`（上一輪實報）的較大者。帳本 `usage/log.rs` `LaneContext` 記 `reopen`、`patched`、`rebased`、`system_hash`。

## 三、做法
1. **Claude GM 線 system 一變就重開**：`plan_turn` 在 `input.lane == Lane::Gm` 時與 Grok／Agy 同一路——`state.applied != frozen_system` → `Reopen(SystemChanged)`，快取存活或過期都一樣（過期時重開與 rebase 成本相同）。另加 `state.snapshot != state.applied` 也重開，用來清掉升級前「快取內補過、尚未 rebase」的舊 GM 線；已 rebase 過的線歷史裡可能仍有舊補丁，不另辨識〔模型判斷·未裁決〕。
2. **system 沒變照舊續聊**：相等時照舊 `Resume`（無補丁），水位、快取壽命、帳本邏輯不動。world.md／卡沒有動態巨集、或巨集展開結果與上輪相同的桌，送出內容與現在逐字相同。
3. **重開時刪掉被取代的舊 Claude session 檔**：GM 線計畫為 `Reopen`（含 `plan_turn` 的各種原因與迴圈裡續聊失敗降級的 `ResumeFailed`）且舊線 provider 是 claude（看舊線，不看本輪 provider，Claude→Grok／Agy 也刪）時，先從 store 拿掉並落檔，**落檔成功才**刪舊 session jsonl（依舊 provider 清理的 helper，不用只看本輪 provider 的 `abandon_session`），刪檔失敗只記 `cleanup-failed` 不擋本輪。避免常重開的桌每輪留一份整幕逐字稿檔。
4. **帳本**：不加新原因，照舊 `system-changed`；GM 線不再出現 `patched`／`rebased`。
5. **不動的部分**〔模型判斷·未裁決〕：
   - Claude 角色共線（每輪抹機密段那種）的補丁／rebase 照舊：它的 system 已把含動態巨集的公開設定移出共用快照，剩下的變動是改卡、改世界書這類低頻操作，補丁換得到快取。
   - API／codex 單發 GM 不動：無狀態、每輪整包重送。
   - 回合尾段（見 tail-worldbook-erase）。

## 四、量測
- 估計：`gm_request` 與實送走同一組裝，不必改；但量測用中性巨集與固定亂數，不保證與實送逐字一致。
- 補丁疊加消失後，GM 線實報與估計的差仍包含歷史裡舊尾段累積造成的部分；中性巨集、固定亂數、token 估算也會造成差。

## 五、測試清單
- `lanes/scaffold_tests.rs` `upgrading_scaffold_patches_or_rebases_claude_and_reopens_grok_agy`（:882）：改成 GM 的 Claude 快取內／過期都 `Reopen(SystemChanged)`、追上後續聊；另寫一份 `Lane::Chars` 版保住 Claude 補丁與 rebase 路徑覆蓋（不可只換 lane）。
- `scaffold_baseline` 三語系：本案不動組裝，預期完全不變；有差異先查原因，不直接重產。
- `transport/turns.rs:795-812`、`transport/context.rs` 約 `:440` 的 GM 組裝測試：尾段不動，確認照過。
- `plan_turn` 單元（直接換 world.md 內容，不靠亂數）：
  - Claude GM：快取內 system 變 → 重開、無補丁；過期 system 變 → 重開而非 rebased；`snapshot != applied` 的舊線 → 重開；首輪原因是 `first-turn`。
  - system 相同：TTL 內、TTL 外、只改狀態或導演指示、正常回覆水位推進，四種都 `Resume` 同 session、`patch: None`、`rebased: false`。
  - Grok／Agy GM、Claude 角色線既有測試照過。
- 假 claude 實跑（`lanes/tests`）：
  - GM 線換 world.md 三輪：每輪新 session、送出 system 只有本輪內容、prompt 無補丁標頭；帳本 `reopen` 首輪 `first-turn`、之後 `system-changed`，`patched`／`rebased` 皆無，`system_hash` 每輪不同；被取代的舊 session 檔已刪。
  - 第四輪 system 不變：續用第三輪 session、只送增量正文、帳本無 `reopen`、`system_hash` 與上輪相同。
  - 舊 session 檔刪不掉：本輪照常完成、帳本記 `cleanup-failed`。
  - Claude→Grok 切換：GM 線重開時舊 Claude session 檔被刪。
  - 續聊失敗降級（`ResumeFailed`）：舊 Claude session 檔被刪。
  - 落檔失敗：舊檔不刪。
- `chat_assembly`：world.md 含巨集但兩輪展開結果相同（如 `{{user}}`）→ 第二輪續聊。

## 六、驗收
- `npm run verify` 全綠。
- 測試通道（claude 低階檔位）：GM 推進數輪，中途改 world.md 一次；以帳本 `system_hash` 判斷——hash 變的那輪 `reopen: system-changed`、沒變的輪續聊無 `reopen`；首輪 `first-turn`；`~/.claude` 專案目錄裡被取代的舊 session 檔已刪。
- 文件同步：`worldbook-st-trigger-parity.md:411` 標已處理；`:135` GM 線跟著變就重開；`:261`「GM 的 system…照既有補丁／重開機制處理」改成 GM 線 system 一變就整線重開；`lanes/mod.rs` `plan_turn` 註解。
- 已知差異：常重開的桌，GM 歷史是重開時壓平的整段逐字稿，不是多輪對話（與 API 單發、Grok 現行相同）。

## 七、尾段疊加（不在本案）
GM 線與角色共線尾段世界書回合後不抹、每輪疊一份：作者裁決回合後抹掉〔作者裁決 2026-10-11〕，另案 [tail-worldbook-erase](../tasks/tail-worldbook-erase.md)。

## 八、已知問題（併入待辦 [solo-private-dynamic-reopen](../tasks/solo-private-dynamic-reopen.md)，不在本案）
- 單人在場 `Hoist::All` 的 Claude 角色線把私設（`own.private`）提進凍結 system，但重開指紋只算世界書段、沒算私設（`transport/turns.rs:271-276`、`:298-307`；`chat_assembly.rs:377-383`）；私設含動態巨集時同樣每輪走補丁疊加。
- 單人在場 `Hoist::All` 因世界書變動重開時同樣會留下舊 Claude session 檔。

## 九、施工結果
- 實作：`lanes/mod.rs` `plan_turn`（GM 線與 Grok／Agy 同路）、`retire_claude_gm_session`（plan 重開與續聊失敗降級兩處呼叫）、`remove_claude_session`。測試：`lanes/tests/gm_restart.rs`（7）、`lanes/tests/grok.rs` Claude→Grok、`scaffold_tests.rs` GM／角色線兩版；scaffold baseline 未變。
- verify 12 步綠；cargo test 1566、vitest 1141、web vitest 774。
- 測試通道（claude-haiku-4-5，空桌 GM 旁白四輪）：第 1 輪 `first-turn`、第 2 輪續聊同 hash、改 world.md 後第 3 輪 `system-changed`（hash 變）、第 4 輪續聊同 hash；無 `patched`／`rebased`／`cleanup-failed`；舊 session 檔已刪、現行檔在且無補丁。haiku 提示太短未達快取門檻，續聊輪 `cache: zero` 與本案無關。
