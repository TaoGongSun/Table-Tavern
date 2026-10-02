# 額度快取紀錄整理＋角色線抹寫丟線

Status: todo（2026-10-06 Grok 額度恢復後開工，四家通道都實跑）〔作者裁決 2026-10-02〕

## Summary
一案做兩件事：

1. **額度快取紀錄整理**：四家通道（claude／codex／agy／grok）加 API 實跑，對照帳本（`~/Documents/TableTavern/prompt-cache.jsonl`）與額度分頁，查清楚哪些線真的有快取、哪些沒有，各種狀態（命中、零命中、不回報、首輪、重開原因等）該掛什麼標籤、顯示什麼。
2. **角色線抹寫丟線**（原 chars-lane-rewrite-drop）：claude 角色線回合後抹寫 session 檔（`apply_rewrite`）失敗就整條丟線（[lanes/mod.rs](../../src-tauri/src/lanes/mod.rs) 兩處 `drop-lane`／`rewrite-failed`），回覆照常、下一輪冷開白花錢。失敗原因被 `Err(_)` 吞掉，不知道是 `find_user_line_with_segment`／`erase_user_segment`／`prefix_last_assistant` 哪段。帳本至今只有一筆（2026-09-03 18:42:06，桌 `01KZ54TYVTKS3930H476ETWF2M`）。

## Next action
先讓抹寫失敗原因落帳本，再四家實跑；claude 角色連講兩三輪看會不會再丟線。

## Constraints
- 與 [non-claude-real-cache](non-claude-real-cache.md)、[no-cache-model-optout](no-cache-model-optout.md)、[vendor-prefix-floor](vendor-prefix-floor.md)、[prompt-cache-optimization](../handoffs/prompt-cache-optimization.md)、[api-cache-visibility](../handoffs/api-cache-visibility.md)、實測佇列第 7 項（額度分頁）範圍相鄰，開工時先劃邊界。
