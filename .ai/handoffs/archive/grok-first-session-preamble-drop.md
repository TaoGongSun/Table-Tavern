# grok-first-session-preamble-drop — 新 grok-home 第一通就丟線

## 根因（2026-10-28 grok 1.0.70 重現）
新 grok-home 第一次開 session 時，bundled skills 在第一通推論中途才裝好（unified.jsonl 推論期間 `skills_reload`），session 開頭沒有技能清單。回合結束後 grok 在 assistant 之後補一則 `type=user`、無 `prompt_index`、`synthetic_reason=system_reminder` 的技能清單，抹寫把它當預期外項目丟線。這則在 updates.jsonl 沒有對應項目（本輪只有 user_message_chunk／agent_message_chunk／turn_completed）。`-r` 續聊時它原位保留，新 prompt 接在後面。

## 定案（Sol、opus-review 共識）
`rewrite_chat_history` 放行 assistant 之後、位於檔尾的一則回合尾提醒（條件見 [plans/grok-shared-lane.md](../../plans/grok-shared-lane.md) chat_history 段），原位不動；assistant 之前、第二則、提醒之後還有項目都照舊丟線。

## 已知限制
- 冷啟動第一通快取只有約 128 tokens，屬 grok 伺服器端行為，不處理〔作者裁決 2026-10-28〕。
- 放行的提醒若夾帶機密段，靠 `scan_residue` 擋下丟線；它只抓得到完整原文，被截斷或改寫過的片段抓不到。

## 狀態
已結案進 main（verify 12 步綠；Sol、opus-review 驗收 PASS）。
