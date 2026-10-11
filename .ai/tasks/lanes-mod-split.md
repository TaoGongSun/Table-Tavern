# lanes/mod.rs 拆檔

Status: todo

## Summary
`src-tauri/src/lanes/mod.rs` 已 1525 行。依功能拆：`plan.rs`（plan_turn、ReopenReason、指紋函式、next_cache_ttl）、`cleanup.rs`（revoke_grok_lane、retire_claude_gm_session、abandon_session、remove_claude_session、settle_abort），run_turn 留在 mod.rs，可回到 1000 行以下。純搬移，不改行為。

## Next action
未排程。機械搬移，可派 Sonnet；verify 全綠即可。
