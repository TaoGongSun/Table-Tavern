# 角色卡路徑匯入時，機制帳本把認不出的條目報成已接管

Status: todo（2026-10-27 製作乾淨測試卡時離線驗出）

## Summary
同一張勇者替代卡（`TestCards/clean-hero-roster.json`）走世界書路徑匯入，帳本是接管 12、跳過 6，正確；走角色卡路徑是接管 18、跳過 0，6 條認不出的 EJS 被列成已接管。根因：角色卡路徑先由 `import_mechanism`（`src-tauri/src/import/card.rs:374`）寫 skipped，再寫世界書（`card.rs:385`，紀錄在 `book_import.rs:193-197`）時寫 absorbed，`read_ledger` 同標題只取最新一筆。違反「世界書路徑要跟角色卡對等」。影響：用角色卡匯入時，玩家看不到哪些條目沒被接管。

## Next action
修法方向：兩條路徑寫帳本的順序與語意對齊，補「兩條路徑對同一張卡帳本相同」的回歸測試。
乾淨測試卡（`TestCards/clean-orc-cave.json`、`clean-hero-roster.json`，產生器在 `TestCards/clean-cards-gen/`）納入版控當回歸夾具〔作者裁決 2026-10-27〕：放哪依 docs/STRUCTURE.md 判斷，連同預期值做快照比對測試，verify 自動跑。
