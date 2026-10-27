# 沒重構的顯示腳本桌換幕後把原卡主角色自動隱藏

Status: todo（2026-10-27 介面接管拍板材料調查時發現）

## Summary
換幕結算只看 present 與「是否接管桌」（`src-tauri/src/data/scene/presence.rs:37-46`），不看卡有沒有顯示腳本。CardFormat 回合走 `card_format_turn`（`chat_assembly.rs:253-257`），不要求 ```state 圍欄，卡的狀態區塊頂層若沒有「在場／在場人物」類別名鍵，present 永遠缺席，主角色卡一換幕就被設成 auto_hidden、掉出 system。現有測試 `missing_present_skips_settlement_only_on_takeover_tables`（presence.rs:142-184）的 plain 列把此行為寫成預期；CardFormat 桌落進 plain 列是讀程式推論，未實機。

## Next action
修法方向：結算的守門條件從「接管桌」改成「本桌沒被要求報 present」（非 Narration 桌）。與 character-presence 的「名冊與 present 誰優先依桌型分」同一個條件，開工時一併看。先實機或測試通道確認一次 CardFormat 桌換幕後主角色被隱藏。
