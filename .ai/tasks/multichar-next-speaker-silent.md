# multichar-next-speaker-silent — 多角色桌點名後角色不接話

## Summary
實測佇列第 19 項②（2026-10-28，Sonnet CLI 3 回合）：多角色（非介面）桌 GM 每輪寫「下一位：X」，玩家對 GM 發言後角色 0 次接著開口。作者裁決的「停用角色點名」只限介面桌，非介面桌應會接話，疑似回歸。證據：scratchpad `vq/19-characters-gm-session.jsonl`、`vq/19-characters-transcript.jsonl`（session 暫存，可能已清）。

## Next action
未排程；先在測試通道重現，追點名到角色開口的流程。
