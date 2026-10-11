# WorldEditor 重構重設測試偶發失敗

Status: todo

## Summary
`src/features/worldbook/WorldEditor.refactor-apply.test.tsx`「a clean existing draft is closed before the reset is sent and never written」在 verify 中偶發失敗（:296 `formOpenAtReset` 預期 false 得 true）；2026-10-11 takeover-mvu-path-mismatch 分支（沒改前端）跑 verify 時出現一次，單跑 5/5 綠、重跑全綠，判為負載下的時序問題。

## Next action
未排程；再遇到或排到時查測試是否依賴未等待的非同步關閉。
