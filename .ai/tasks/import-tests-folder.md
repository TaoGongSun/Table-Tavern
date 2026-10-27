# import/ 測試檔收進子資料夾

Status: todo（2026-10-27 mechanism-ledger-card-path 結構體檢時提出）

## Summary
`src-tauri/src/import/` 現有 24 支 .rs，其中 11 支是測試檔，超過 docs/STRUCTURE.md「單一資料夾約 20 檔就考慮分類」。`refactor/tests/` 已有子資料夾先例。依 STRUCTURE.md，這屬大規模搬家，不在功能案裡順手做，故獨立立案。

## Next action
未排程；MVU 相容段開工前的結構體檢一併判斷（MVU 段會碰 import/）。純搬檔一筆 commit，搬前後核對測試收集數與名稱相同。
