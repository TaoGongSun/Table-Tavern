# flat-field-glyph-dup — 沒重構非增量桌，模型用另一字形寫鍵會在平欄長出重複欄位

## Summary
stage-card-in-speaker-list 查證（2026-10-28，只讀碼未重現）：`mechanism/ledger.rs` 的 `apply_block_typed` 在 `fence_tree_target` 回 None（非增量桌、或鍵不在狀態樹）時直接用模型原字 `table.insert`，沒做簡繁折疊。卡片開場白用簡體鍵（剧情阶段）、GM 寫繁體（劇情階段）時，平欄會出現兩份。重構增量桌有折疊兜底，不受影響。

## Next action
未排程。先寫紅燈測試重現；修法方向：同層平欄精確優先、簡繁折疊唯一命中才沿用原鍵，歧義拒寫並回饋，不合併既有重複鍵。

## Constraints
- 會改狀態寫入語意，獨立測試、可單獨回退。
