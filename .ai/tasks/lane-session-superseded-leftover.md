# lane-session-superseded-leftover — 被取代的 grok lane session 沒刪

## Summary
2026-10-28 grok-image-original-leftover 實測時發現：測試 root 裡兩個已被取代的 lane session（8babe9fc、ffb7f3e3）目錄還在，原因沒查。lane 重開後舊 session 應刪或抹寫。

## Next action
未排程。先確認 agy-home-cleanup 的 7 天 TTL（grok 原生 `storage.cleanup_ttl_days`）結案後是否已涵蓋；涵蓋就只剩「重開當下沒刪」是否要即時刪，再查哪條重開路徑漏刪。
