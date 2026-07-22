# Task
Task-ID: sample-world-i18n
Title: 範例桌內容依語系產生（en 使用者首開拿英文範例桌）
Status: todo
Created: 2026-07-23T00:40:00+08:00
Updated: 2026-07-23T00:40:00+08:00

## Summary
首開自動建立的範例桌「迷霧酒館（範例）」（data.rs `create_sample_world`：桌名、3 張角色卡、world.md、開場旁白）目前寫死中文。UI 語系切換（ui-i18n-switch）完成後，英文使用者首開仍拿到中文範例內容。目標：依 `config.preferences.language` 產生對應語言的範例桌；已存在的桌是資料，不回頭改。

## Next action
- 把 create_sample_world 的範例內容抽成 zh-TW／en 兩份，依 config 語系選用；注意首開時 config 可能還是預設值（zh-TW），需想清楚「先選語言還是先建桌」的順序（例如首開偵測系統語系，或語言切換時若範例桌仍是空桌未動過就重建）

## Constraints
只影響新建的範例桌；使用者已動過的桌一律不改。後端錯誤訊息的 i18n 不在本任務範圍。
