# Agy 一角一線狀態區塊消失重開

Status: 已結案（2026-10-11 合併 main）

## 結論
比照 Claude 單人線重開（三方審查 PASS）。`run_turn` 原把 `single_owner` 與 `has_state_block` 綁在一起歸零，是 claude-resume-tail-cache「agy 不套未抹線判定」的連帶；Agy 不需要 OwnerChanged／ModeChanged（線名按角色分、私設提進 system），但它不抹尾段，狀態區塊永久留在歷史，整塊消失時舊值撤不掉，正是 StateBlockGone 要擋的。

## 做法
- `run_turn` 算一次 `keeps_tail`＝角色線且（claude 單角色 `single_owner` 有值，或 Agy）。`single_owner` 照舊只留給 claude 角色線；`has_state_block` 只在非 `keeps_tail` 時歸零；落檔 `had_state_block = keeps_tail && has_state_block`。
- `plan_turn`：StateBlockGone 緊接 owner 區塊（OwnerChanged、ModeChanged）之後、WorldbookChanged 之前，不再限有 owner。claude 線行為不變。
- Agy 重開沿用開新 conversation，沒有 session 檔要清。

## 已知限制／差異〔模型判斷·未裁決〕
- 升級前的 Agy 落檔明寫 `had_state_block=false`：若升級首輪區塊已消失，舊值留到這條線因其他原因重開（換場等）、或區塊恢復後再消失為止；之後新出現又消失的才正常重開。依「發佈前不保舊桌」不處理。
- GM 線「目前狀態」段整塊清空時舊值留在歷史（三家皆同、極罕見、無隱私問題），不處理、不另立案。

## 測試
- `lanes/tests/owner.rs`：`agy_lane_reopens_when_state_block_vanishes`（single_owner=None，有→有續用、有→無開新且重帶 system、reason＝state-block-gone、無→無續用，逐輪驗 `had_state_block`）；`agy_lane_drops_owner_but_keeps_state_block_rule`（故意填 owner 被清、在場變多續用、區塊消失仍重開）；`shared_claude_lane_ignores_state_block_vanishing`、`gm_lane_ignores_state_block_vanishing`、`agy_gm_lane_does_not_record_state_block`；`plan_turn_owner_rules_in_fixed_order` 補 owner 為 None 的 state-block-gone。
- `lanes/tests/grok.rs`：`grok_chars_lane_ignores_state_block_vanishing`。
