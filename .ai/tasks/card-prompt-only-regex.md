# card-prompt-only-regex — 後端套卡片 promptOnly 正則（對齊 ST 送模型前的歷史處理）

## Summary
greeting-context-st-parity 查證（2026-10-28）：ST 生成時對整份歷史套卡片的 promptOnly 正則腳本（含 minDepth／maxDepth、placement），我們後端完全不套。結果是 NorthHall（剝 `<Status_block>`）、MVU 卡（剝 UpdateVariable）等會多送 ST 不送的狀態欄或更新區塊；GM 歷史送 raw 還是 text（API 送 text、CLI lane 送原文）也該一起決定。

## Next action
未排程。開工先盤點測試卡的 promptOnly 腳本用法，評估 JS 相容正則引擎（方向：Rust 的 ECMAScript 正則 crate `regress`，不拿 `regex` 硬套）；替換語法、旗標、placement、深度、執行順序都要獨立驗收。照「模仿費工就不模仿」先估工再拍板。

## Constraints
- 不改匯入／匯出；只作用於組提示詞那一步。
- 世界書掃描與巨集要用哪個版本另行決定，不可讓掃描多觸發。
