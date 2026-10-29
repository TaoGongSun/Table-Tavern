# card-prompt-only-regex — 後端套卡片 promptOnly 正則（對齊 ST 送模型前的歷史處理）

## Summary
greeting-context-st-parity 查證（2026-10-28）：ST 生成時對整份歷史套卡片的 promptOnly 正則腳本（含 minDepth／maxDepth、placement），我們後端完全不套。結果是 NorthHall（剝 `<Status_block>`）、MVU 卡（剝 UpdateVariable）等會多送 ST 不送的狀態欄或更新區塊；GM 歷史送 raw 還是 text（API 送 text、CLI lane 送原文）也該一起決定。

## Next action
2026-10-29 輕防重審：縮小〔模型判斷·未裁決〕（依「模仿要費工變爛時不模仿」）。最小版＝用 `regress`（ECMAScript 正則）只套 TestCards 裡實際出現的 promptOnly 形狀（預計多半是整段剝掉、替換成空字串、不分深度）；開工第一步盤點 TestCards 用法，確認最小版涵蓋大宗。GM 歷史送 raw 還是 text 同案定。
刪掉的部分成已知限制：只支援常見的 promptOnly 剝除寫法，minDepth／maxDepth、各種 placement 組合、`$1` 以外的替換語法不支援。

## Constraints
- 不改匯入／匯出；只作用於組提示詞那一步。
- 世界書掃描與巨集要用哪個版本另行決定，不可讓掃描多觸發。
