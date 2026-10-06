# 額度快取紀錄整理＋角色線抹寫丟線

Status: in-progress（2026-10-06 開工，分支 `usage-cache-audit`；四家通道都實跑〔作者裁決 2026-10-02〕）

## Summary
一案做兩件事：

1. **額度快取紀錄整理**：四家通道（claude／codex／agy／grok）加 API 實跑，對照帳本（`~/Documents/TableTavern/prompt-cache.jsonl`）與額度分頁，查清楚哪些線真的有快取、哪些沒有，各種狀態（命中、零命中、不回報、首輪、重開原因等）該掛什麼標籤、顯示什麼。
2. **角色線抹寫丟線**（原 chars-lane-rewrite-drop）：claude 角色線回合後抹寫 session 檔（`apply_rewrite`）失敗就整條丟線（[lanes/mod.rs](../../src-tauri/src/lanes/mod.rs) 兩處 `drop-lane`／`rewrite-failed`），回覆照常、下一輪冷開白花錢。失敗原因被 `Err(_)` 吞掉，不知道是 `find_user_line_with_segment`／`erase_user_segment`／`prefix_last_assistant` 哪段。帳本至今只有一筆（2026-09-03 18:42:06，桌 `01KZ54TYVTKS3930H476ETWF2M`）。
3. **順帶收兩條快取尾巴**〔作者裁決 2026-10-02〕：
   - prompt-cache-optimization 的保溫「離開提醒」（保溫連三次沒回應＋紀錄超過 8000 字元才亮）實跑時等滿 12 分鐘驗一次。
   - 收 OpenRouter 真實命中率，作為保溫設計的參數依據（原 api-cache-visibility 留下的下一步）。

規格、邊界、實跑步驟與預估花費：[plans/usage-cache-audit.md](../plans/usage-cache-audit.md)。

## Next action
計畫第 1 輪審查（Sol）五項必改已併入，等第 2 輪；審過後先施工抹寫失敗原因落帳本、跑 verify，再照計畫四家＋OpenRouter 實跑對帳。

## Constraints
- 與 [non-claude-real-cache](../tasks/non-claude-real-cache.md)、[no-cache-model-optout](../tasks/no-cache-model-optout.md)、[vendor-prefix-floor](../tasks/vendor-prefix-floor.md)、[prompt-cache-optimization](prompt-cache-optimization.md)、[api-cache-visibility](archive/api-cache-visibility.md)（額度分頁 2026-10-02 實機已過）範圍相鄰，邊界表見計畫檔。
- 測試通道全機一把鎖，與其他子代理輪流用。
- Grok 額度 2026-10-06 22:00 才恢復，之前不對 grok 送任何請求；grok 登入檔可複製進測試 root、跑完刪〔作者裁決 2026-10-06〕。
- 「各狀態掛什麼標籤、顯示什麼」是玩家看得到的設計：只寫建議、列待使用者決定，不施工 UI。
