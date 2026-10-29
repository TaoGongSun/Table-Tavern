# lane-session-superseded-leftover — 被取代的 grok lane session 沒刪

## Summary
2026-10-28 grok-image-original-leftover 實測時發現：測試 root 裡兩個已被取代的 lane session（8babe9fc、ffb7f3e3）目錄還在，原因沒查。lane 重開後舊 session 應刪或抹寫。

## Next action
2026-10-29 輕防重審：只記已知限制、暫不動工〔模型判斷·未裁決〕。前置 agy-home-cleanup 已結案（grok 寫入 `storage.cleanup_ttl_days=7`、單發結束刪 session），殘留目錄最多多留幾天。
剩一步：確認 grok 的 TTL 也涵蓋 lane session 目錄；涵蓋就在 [agy-home-cleanup](../plans/agy-home-cleanup.md) 已知限制加一句「lane 重開時舊 session 不即時刪，靠 TTL 回收」，不查漏刪路徑。
