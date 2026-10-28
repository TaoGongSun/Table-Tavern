# codex-floor-sandbox-group — codex 自帶底線依沙盒模式不同，分組沒算進去

## Summary
non-claude-real-cache 實測（2026-10-28，codex 0.164.0、CLI 預設模型）：單發量到 cached 12,288，vendor-prefix-floor 記的是 11,008，差 1,280。推測（未驗證）來自沙盒模式不同（聊天單發 read-only、帶工具 workspace-write）。`usage/floor.rs` 分組沒含沙盒模式又取最小值，可能把 12,288 誤判成「我方中了 1,280」。另：codex 這種「有回報但全是底線」的情況，no-cache-model-optout 用「回報 0／沒回報」會誤當有快取。

## Next action
未排程。先驗證差額來源（兩種沙盒模式各打一通比 cached），成立就把沙盒模式納入帳本分組或記錄欄；同步提醒 no-cache-model-optout 判定要扣底線。

## Constraints
- 帳本只加可選欄，舊行照讀。
