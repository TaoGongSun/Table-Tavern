# Task
Task-ID: transcript-export
Title: 一鍵下載跑團紀錄（劇情歷史匯出）
Status: completed
Created: 2026-07-23T01:10:00+08:00
Updated: 2026-07-23T22:40:00+08:00

## Summary
逐字稿已在背景落地（`worlds/<world>/transcript/<scene>.jsonl`，data.rs `append_transcript`），但玩家拿不到可讀版本。目標：桌內加一個匯出鈕，把整桌（全部場景依序）轉成可讀的 Markdown（發言者、旁白、系統訊息格式化，含桌名與日期），讓玩家自己存檔留念或分享。

## Next action
- 無。2026-07-23 使用者實測另存對話框存檔通過（附截圖），結案

## Constraints
只讀不改正典資料；匯出檔是衍生品，不回頭匯入。格式先 Markdown 一種就好（YAGNI）。
