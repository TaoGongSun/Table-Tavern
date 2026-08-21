# API 路徑改走 chars 共線：讓換角色不再打散前綴快取

Status: todo

## Summary
claude 路徑早已做過「全角色共用一條線」，API／codex／grok 被 [lib.rs:2201](../../src-tauri/src/lib.rs) 一行 `if chat_transport(&config) == "claude"` 擋在外面，於是每換一個角色就整包重算前綴快取（2026-08-21 實測：同角色連續 94% 命中，換角色掉到 64＝全滅）。查證已完成、設計未拍板，規格與四個待決問題見 [.ai/plans/api-shared-lane.md](../plans/api-shared-lane.md)。

## Progress
- 已確認 claude 路徑的 `chars_lane_system`／`chars_lane_turn` 是現成純函式（已有測試），API 路徑沒去呼叫。
- 已找出真正的障礙：不只 system 不同，**transcript 的 role 分配也是角色專屬的**（自己說的是 assistant、別人說的是帶前綴的 user），光換 system 不夠。
- 已確認 API 無狀態是優勢：claude lane 那套「回合後把私設從 session 檔抹掉」完全不用實作。
- 已量出代價：角色卡合計約 1,810 tokens，共線後每次呼叫輸入多約 1,500（+19%），換角色不再全額重建。

## Next action
開工前先跟 Sol 討論 plan 文末四題，尤其第 1 題（role 分配改成「全部 assistant ＋名字前綴」對第三方模型的副作用）與第 2 題（只換 system 不動 role 規則的中間方案，20% vs 95%）。

## Constraints
單角色桌沒有增益（一桌一張是主流玩法），可能需要「角色數 ≥ 2 才走共線」的條件分支；codex／grok 走同一條路徑會一起改變，而它們的快取行為沒量過。
