# character-presence — 卡片自訂名冊欄位接到在場機制

## Summary
2026-08-11 orc-cave 實測後立案：卡片狀態欄裡「駐留角色」這類名冊欄位只是 AI 回報的清單，沒接到任何機制。在場機制本身已由 ai-card-refactor 包 4 做完（app 的 `present` 欄位驅動在場過濾、自動上下場、換幕結算，見 [CARD-REFACTOR-SPEC 包 4](../reference/CARD-REFACTOR-SPEC.md)），本案只做「卡片自訂名冊欄位接到既有在場機制」，不重做包 4〔作者裁決 2026-10-02〕。

## Next action
開工前拍板：怎麼認出哪個欄位是名冊（重構時標記？欄位名比對？），以及名冊與 `present` 不一致時誰優先。

## Constraints
- 手動封存是玩家的決定，永不被 AI 自動拉回（包 4 拍板）。
- 幕中不動 system（快取紅線），增減一律走歷史 append 或換幕結算。
