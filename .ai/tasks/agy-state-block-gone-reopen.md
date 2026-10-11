# Agy 一角一線狀態區塊消失不重開

Status: todo

## Summary
Agy 一角一線（不抹尾段）在 `run_turn` 把 `has_state_block` 歸零（`lanes/mod.rs` 約 :1116），狀態區塊整塊消失時不走 `StateBlockGone` 重開，舊狀態區塊留在不抹的歷史裡（`lanes/tests/owner.rs` 約 :943 的測試現況斷言照常續用）。Claude 單人線的同類情況已會重開。

## Next action
未排程。先確認 Agy 歸零的原因，再決定比照 Claude 未抹線重開或列已知差異。
