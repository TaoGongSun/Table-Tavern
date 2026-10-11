# 重構套用整條改寫條目，開著的既有條目草稿會被自動存蓋回

Status: todo

## Summary
import-refresh-world-editor 審查發現（既有問題）：世界設定裡套用重構卡（PNG），`WorldEditor.tsx` 的 `refreshAfterApply` 只重讀條目、帳本、名單，不處理開著的既有條目草稿。重構會整條改寫條目；玩家若開著那條草稿且改過，切條目自動存（`update_entry_fields`）會把整份舊草稿蓋回去，重構結果遺失。匯入與撤銷那兩條已用 WorldChange＋待定基處理同類問題，可比照。

## Next action
未排程。
