# play-folder-split — src/features/play/ 檔數過多

## Summary
`src/features/play/` 在 undo-two-clicks-summary-line 加了 undo-stack 兩支後達 27 支，超過 STRUCTURE.md「一個資料夾超過約 20 檔考慮開子資料夾」的方向；useChatController.ts 也約 1,011 行。

## Next action
未排程；照 docs/STRUCTURE.md 依功能分子資料夾（純搬檔，測試收集數不變），順便評估 useChatController.ts 依功能拆。
