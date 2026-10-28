# refactor-undo-ai-apply — AI 重構套用不記收據，「復原上次匯入」撤到更早的匯入

## Summary
statusbar-only-fields 實測（2026-10-28）發現：測試桌做完 AI 重構後按「復原上次匯入」，撤到的是更早那筆世界書匯入。結果狀態樹清空了，但機制表、refactor_mode、statusbar_retired 和 8 張角色卡都留著。匯入重構卡再套用的那條路有收據，可以正常還原。

## Next action
未排程。先確認 AI 重構套用不記收據是刻意設計還是遺漏，再決定要補收據，還是讓復原鈕跳過重構這一步。
