# 角色線續聊被作廢：每輪冷開、快取一次都中不到

Status: todo

## Summary
2026-09-03 transport-split 的實機 smoke test 順手抓到：claude 通道的角色線發完一輪後，帳本落一筆 `event: drop-lane`／`reason: rewrite-failed`，續聊 session 被整條丟棄。

現象是「回覆正常、只有錢不對」——[lanes.rs:583](../../src-tauri/src/lanes.rs:583) 判定 session 檔抹寫失敗就作廢該線，本輪回覆照常送回，畫面看不出異狀，但下一輪角色發言重新全量冷開。同一時間 GM 線 cached 5877／prompt 7466，角色線 `cached_tokens: 0`、`reason: first-turn`。

抹寫動作在 `apply_rewrite`：把私密段落從 session 檔的 user 行抹掉、或替最後一則 assistant 加前綴，任一步失敗就丟線。實際是哪一步失敗、為什麼失敗，尚未查。

歷來帳本（`~/Documents/TableTavern/prompt-cache.jsonl`）只出現過兩次 drop-lane，另一次是 2026-08-06 的 GM 線 `ping-truncate-failed`，本次是第一次 `rewrite-failed`，樣本只有一筆。

## Progress
2026-09-03 立案，尚未動工。證據：該桌 `01KZ54TYVTKS3930H476ETWF2M`，帳本時間戳 `2026-09-03 18:42:06` 那兩筆。

## Next action
開工首步＝重現並定位：連續讓角色發言兩三輪，看每輪是不是都落 drop-lane，再確認 `apply_rewrite` 裡失敗的是 `find_user_line_with_segment`／`erase_user_segment`／`prefix_last_assistant` 哪一段。目前錯誤被 `Err(_)` 吞掉不落原因，可能要先讓它把失敗原因寫進帳本才查得動。

## Constraints
- 只影響費用不影響回覆內容，不是急件。
- 與 `grok-cache-miss`（grok 角色線）、`api-shared-lane`（API 角色線共線）都碰角色線快取，開工前確認三案的邊界不要重工。
