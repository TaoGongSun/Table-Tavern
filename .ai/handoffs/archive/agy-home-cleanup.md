# agy-home-cleanup — app 專用 CLI 家目錄（agy／grok／codex）的清理

Status: 已結案（2026-10-28 合併 main，包 1、包 2 各一筆）

app 自己的 agy-home、grok-home、codex-home 不再無限累積對話：agy 與 codex 由 app 啟動回收（標記清除、7 天）＋lane 重開與單發即時刪；grok 寫入自帶的 `storage.cleanup_ttl_days=7`、單發結束刪 session、每次呼叫結束刪 prompt_history、在途歸零刪搜尋索引；codex 一律走官方 `codex delete --force`、行程內串行。方案、查證依據與已知限制見 [plans/agy-home-cleanup.md](../../plans/agy-home-cleanup.md)。
