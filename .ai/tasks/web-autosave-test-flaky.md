# web 自動存檔測試偶發失敗

Status: todo

## Summary
`web/src/features/chat/autosave.test.tsx`「first slot must land before the first dispatch」在 verify 中偶發失敗（:184 streamChat 預期呼叫 1 次得 0 次）；2026-10-11 takeover-text-multiline 分支（沒改 web）跑 verify 時出現一次，單跑 5/5 綠、重跑全綠，判為負載下的時序問題。

## Next action
未排程；再遇到或排到時查 settle() 是否等足非同步存檔。
