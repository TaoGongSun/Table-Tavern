# 額度快取紀錄整理＋角色線抹寫丟線

Status: todo（2026-10-06 Grok 額度恢復後開工，四家通道都實跑）〔作者裁決 2026-10-02〕

## Summary
一案做兩件事：

1. **額度快取紀錄整理**：四家通道（claude／codex／agy／grok）加 API 實跑，對照帳本（`~/Documents/TableTavern/prompt-cache.jsonl`）與額度分頁，查清楚哪些線真的有快取、哪些沒有，各種狀態（命中、零命中、不回報、首輪、重開原因等）該掛什麼標籤、顯示什麼。
2. **角色線抹寫丟線**（原 chars-lane-rewrite-drop）：claude 角色線回合後抹寫 session 檔（`apply_rewrite`）失敗就整條丟線（[lanes/mod.rs](../../src-tauri/src/lanes/mod.rs) 兩處 `drop-lane`／`rewrite-failed`），回覆照常、下一輪冷開白花錢。失敗原因被 `Err(_)` 吞掉，不知道是 `find_user_line_with_segment`／`erase_user_segment`／`prefix_last_assistant` 哪段。帳本至今只有一筆（2026-09-03 18:42:06，桌 `01KZ54TYVTKS3930H476ETWF2M`）。

3. **順帶收兩條快取尾巴**〔作者裁決 2026-10-02〕：
   - prompt-cache-optimization 的保溫「離開提醒」（保溫連三次沒回應＋紀錄超過 8000 字元才亮）實跑時等滿 12 分鐘驗一次。
   - 收 OpenRouter 真實命中率，作為保溫設計的參數依據（原 api-cache-visibility 留下的下一步）。

4. **grok 整條線不命中**：grok 角色線 2026-10-06 實測，有一條線換幕重開後連四輪 below-expected（cached 只有 128–1152），同時段別條線正常，帳本是那次測試通道 ttroot 那批。列為本案要查的現象，查完判斷要不要換 session 自救〔作者裁決 2026-10-06〕。

## Next action
先讓抹寫失敗原因落帳本，再四家實跑；claude 角色連講兩三輪看會不會再丟線。

## Constraints
- 與 [non-claude-real-cache](non-claude-real-cache.md)、[no-cache-model-optout](no-cache-model-optout.md)、[vendor-prefix-floor](vendor-prefix-floor.md)、[prompt-cache-optimization](../handoffs/prompt-cache-optimization.md)、[api-cache-visibility](../handoffs/archive/api-cache-visibility.md)（額度分頁 2026-10-02 實機已過）範圍相鄰，開工時先劃邊界。
