# refactor-outcome-fields — 重構產物帶盤點 FIELDS 與不收格子名單

## Summary
statusbar-only-fields（2026-10-28）拆出來的案子，三件事都會改重構產物格式（RefactorOutcome、前端 parseRefactorOutcomeValue、重構卡匯出匯入），一次做完：
1. G3：開場白沒附狀態欄的卡（西幻、寻道太虚、灰岩要塞預設開場），沒有開場格子可以追蹤。把盤點 FIELDS 帶進產物，替沒有開場值的欄位建規則。
2. 不收格子：〔作者裁決 2026-10-28〕地圖與導航清單、記憶與摘要不收，改由正文描述；行動選項、環境與天氣照收。由盤點依用途列出不收名單（UNTRACKED），套用時移除。
3. 兩個新欄位都用 `Option<Vec<String>>`，要分得出「舊產物沒有這欄」和「新產物給空清單」，匯出再匯入要保留空陣列。
本案完成前，地圖、記憶這類格子如果出現在開場狀態欄，會照一般格子追蹤。

## Next action
未排程；前置 statusbar-only-fields 已結案（2026-10-28），可排程。statusbar-only-fields 的計畫檔裡有 UNTRACKED 第 4 版的設計可以參考。
