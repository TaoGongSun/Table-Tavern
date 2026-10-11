# 其他機制錯誤也回饋給 GM

Status: todo

## Summary
`mechanism/apply.rs` 裡「路徑不存在」「對分支增減」等仍記 Error，`build_notes` 只收 Rejected，GM 不知道被擋、下回合照錯（例：`replace /神經狀態/0`、MVU 卡 GM 自創路徑）。takeover-text-field-delta 已把型別不符類改成 Rejected 回饋，這裡處理剩下的 GM 可修正錯誤；帳本現值壞掉維持 Error。

## Next action
未排程。
