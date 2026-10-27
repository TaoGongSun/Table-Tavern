# card-shim-no-zod — 卡片 iframe 的 shim 沒有 z

## Summary
卡片前端自己呼叫 parseMessage 時，zod 桌仍走 MagVarUpdate 的語意，可能接受 zod 路徑會拒收的值。

## 已知線索
- 見 mvu-zod-semantics 規格 §4.4（語意缺口；該案在 `mvu-zod-semantics` 分支施工中）。

## Next action
未排程；排在 mvu-zod-semantics 合併之後。

## 驗收方向
zod 桌卡片前端的 parseMessage 與後端對同一輸入的接受／拒收一致。
