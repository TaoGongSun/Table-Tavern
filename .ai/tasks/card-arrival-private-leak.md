# 角色卡回歸事件把私設漏給同桌其他角色

## Summary

`transport::card_arrival_text` 產生的「角色回歸」事件正文含 `private_md` 全文，`record_card_arrivals` 標 `gm_only: false`，所以它會逐字出現在每一條角色線的歷史裡——同桌其他角色因此讀得到本來只有本人知道的設定。

原始碼註解寫「chars 快照本來就含全卡，回歸事件不算新洩漏」，但 `chars_lane_system` 實查只吐 `public_md`，前提不成立。claude 共線、api／codex／agy 單發、grok 續聊四條路都中。2026-08-22 做 grok-cache-miss 時發現，早於該案。

## Next action

先拍板「回歸事件該讓誰看到什麼」：是拆成公開回歸事件＋GM-only 私設事件，還是回歸事件只留公開設定。定了再看四條路各要怎麼改，並一併決定 grok 現在的「一角一線＋私設提進凍結 system」要保留還是改回共線——grok-cache-miss 的角色線驗收擋在這裡。

## Constraints

- 這是可見性憲法的洞，不是效能問題。
- 已寫進 transcript 的舊事件也帶著私設，要決定舊桌怎麼處理。
