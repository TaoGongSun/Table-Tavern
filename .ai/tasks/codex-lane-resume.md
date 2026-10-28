# codex-lane-resume — codex 改走 lane 續聊，讓我方內容吃到快取

## Summary
non-claude-real-cache 實測（2026-10-28，詳見 [plans/non-claude-real-cache.md](../plans/non-claude-real-cache.md)）：codex `exec --ephemeral` 單發時我方內容零命中（同內容重送也只中 codex 自帶 12,288）；`exec resume` 同一線 3 輪續聊 2 輪中到上一輪整段。推論 codex 以 conversation id 當快取鍵。

## 待拍板（開工前）
- session 存放：寫進使用者 ~/.codex（會混進其 resume 清單、要訂清理）vs `CODEX_HOME` 隔離（auth.json refresh token 輪替兩邊可能互相登出）——需作者裁決。
- 私設隔離：先走 agy 式一角一線；「回合後改寫 session 檔」須先實證 codex 可安全改寫並續聊。
- usage：resume 回累計值，比照 agy usage base 逐輪相減（綁 session、重開歸零、取消後重同步）。
- 固定每輪旗標；觀察到第 4 輪中段重注入 19.5KB skills_instructions，須查明並避免線中段膨脹。

## 驗收方向
重用 lanes::plan_turn；在 2、5、10 分鐘輪間間隔各測 ≥10 輪量實際收益（扣 codex 自帶底線，不硬編 12,288）；偶發不中不觸發重開。
