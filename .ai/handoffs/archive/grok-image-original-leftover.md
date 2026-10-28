# grok-image-original-leftover — grok 生圖原始檔留在 app 的 grok-home

## Summary
grok 每次單發都在 `grok-home/sessions/<群組>/<id>/` 留一整份 session；生圖那份含原始 JPEG（images/1.jpg），每次約 488 KB，app 不清。查證與方案見 [plans/grok-image-original-leftover.md](../../plans/grok-image-original-leftover.md)。

## Next action
已結案（Sol、opus-review 驗收 PASS），無剩餘事項。分支 grok-image-original-leftover、worktree 在主線 scratchpad 的 wt-grok-image-original-leftover。

## Constraints
- 只清 app 自己的 grok-home；只刪圖已經進了圖庫的 session，不跟隨連結。
- 與 agy-home-cleanup 分工：本案即時刪單次生圖；定期按期限清整個家目錄歸對方。
