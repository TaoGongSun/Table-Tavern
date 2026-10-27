# undo-two-clicks-summary-line — 規格

起因：實測佇列第 23 項，「收回上一句」第一下只收變動摘要行，第二下才收 GM 旁白；樹模式桌收掉旁白後留下的登場行帶回合後快照，數值不回滾。

## 拍板（Sol、Grok 審查後主線裁決 2026-10-28）
- **收一樓**（新指令 `pop_transcript_floor`，同一把鎖、一次重寫、回滾一次，回傳實際刪掉的事件、原序）：
  1. 先收尾端連續附屬行：`state_update`、`gm_call`。
  2. 再收一樓：玩家句、GM 旁白、角色回覆、開場白，或任何非附屬、非登場塊的事件（換幕摘要、無標記舊系統行、孤立私設）。
  3. 再往前吃緊貼的登場塊：連續的 `card_arrival`／`person_arrival`／`card_private`，且 `action_id` 與該樓相同（兩邊都要有值，None 不算）。
  - 邊界用 marker 白名單；舊 `pop_transcript`（砍一則）保留。
- **復原整組**（新指令 `restore_transcript_events`）：一次提交照原序寫回，狀態採最後一則快照；前端復原疊改成組，失敗整組留在疊上可重試、不重複追加；沿用後端回傳的新 vars_rev 與換桌世代檢查。
