# 角色卡路徑匯入時，機制帳本把認不出的條目報成已接管

Status: todo（2026-10-27 製作乾淨測試卡時離線驗出）

## Summary
同一張勇者替代卡（`TestCards/clean-hero-roster.json`）走世界書路徑匯入，帳本是接管 12、跳過 6，正確；走角色卡路徑是接管 18、跳過 0，6 條認不出的 EJS 被列成已接管。根因：角色卡路徑先由 `import_mechanism`（`src-tauri/src/import/card.rs:374`）寫 skipped，再寫世界書（`card.rs:385`，紀錄在 `book_import.rs:193-197`）時寫 absorbed，`read_ledger` 同標題只取最新一筆。違反「世界書路徑要跟角色卡對等」。影響：用角色卡匯入時，玩家看不到哪些條目沒被接管。

## Next action
未排程，排在實測佇列梯 2 第 6 項跑完之後（避免實測途中改帳本程式）。修法方向：兩條路徑寫帳本的順序與語意對齊，補兩條路徑對同一張卡帳本相同的回歸測試。可順帶考慮把乾淨測試卡納入版控當回歸夾具（業界 golden-file 做法；目前 TestCards 是 gitignore 的本機檔）〔模型判斷·未裁決〕。
