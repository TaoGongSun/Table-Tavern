# agy 跑長篇重構盤點多半失敗

## Summary

stage-card 包 3 實測（2026-10-28）：agy（Gemini 3.8 Flash Medium，測試根目錄假 HOME）對 5 張卡跑多角色盤點各 3 次，只成功 1 次。失敗有兩種：
- `cli_stalled`（約 2 分鐘被判卡住）；
- 模型自行呼叫工具，在無頭模式被自動拒絕（「工具需要 command 權限」）。

同一 agy 跑短的重構初判（recommend）成功。根因未查，不能斷定與舞台卡改動無關。

## Next action

未排程。先在 main 上不帶舞台卡改動重現，分清是盤點長度、卡住判定門檻，還是 agy 無頭工具呼叫的問題。

## Constraints

- 只能用測試 HOME，不碰作者的 ~/.gemini。
- 逾時以玩家耐性為準，不為慢模型放寬。
