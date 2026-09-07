# Task
Task-ID: ai-workspace-tidy
Title: .ai/tasks/ 逐檔判斷歸屬（62 檔已超過一眼掃得完的量）
Status: backlog
Created: 2026-09-07T00:00:00+08:00
Updated: 2026-09-07T00:00:00+08:00

## Summary

`.ai/tasks/` 累積到 62 個檔，遠超過 CLAUDE.md 訂的「一個資料夾超過約 20 個就分類」。裡面混了三種狀態：真的還沒開工、已經被其他案吸收但檔案還在、以及久到可能已經不成立的。BACKLOG.md 的行數與 tasks/ 的檔數是否一致也沒人核對過。

這是純判斷工作，跟搬程式碼完全不同性質，所以從 [source-structure](../plans/source-structure.md) 切出來。

## Next action
- 未排程。開工首步＝逐檔比對 tasks/ 與 BACKLOG.md，列出三類清單（留下／已被吸收可刪／狀態不明），**不明的那類要逐條問使用者，不得自行判成「維持現狀」後略過**。

## Constraints
- `handoffs/archive/`、`history/`、`DONE.md` 是凍結只讀的存檔，不動。
- 刪任何一個 task 檔前先確認沒有其他文件連向它，不製造死連結。
- 分子資料夾是選項不是目標；若逐檔清完後剩下的量本來就掃得完，就不必分類。
