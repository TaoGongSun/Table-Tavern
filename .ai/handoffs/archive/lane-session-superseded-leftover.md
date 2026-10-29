# lane-session-superseded-leftover — 被取代的 grok lane session 沒刪

## Summary
2026-10-28 grok-image-original-leftover 實測時發現：測試 root 裡兩個已被取代的 lane session（8babe9fc、ffb7f3e3）目錄還在，原因沒查。lane 重開後舊 session 應刪或抹寫。

## 結案（2026-10-29）
不改程式。grok 的 `storage.cleanup_ttl_days=7` 跨所有群組回收閒置 session 目錄（agy-home-cleanup 實測），lane session 也在內；已知限制寫進 [agy-home-cleanup](../../plans/agy-home-cleanup.md)「已知限制」。
