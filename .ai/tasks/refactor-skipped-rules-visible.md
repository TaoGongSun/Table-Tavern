# refactor-skipped-rules-visible — 重構套用略過的規則與觸發要讓玩家看得到

## Summary
playdata-loss-fixes 稽核發現（範圍外舊碼）：重構套用時，對不到狀態樹的規則與觸發由 `keep_rule`／`keep_trigger`（`refactor/apply.rs` 約 609 行）默默略過、只記 log，例如「第 7 天斥候登場」可能根本不生效。三題：玩家會遇到（會）、不做會怎樣（劇情事件不觸發且不知道）、跟地基有關（資料正確）。

## Next action
排在 playdata-loss-fixes 之後；最小版＝結果框顯示略過筆數與名稱。
