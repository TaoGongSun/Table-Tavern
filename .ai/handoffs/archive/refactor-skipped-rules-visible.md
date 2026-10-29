> 結案 2026-10-29：進 main，verify 13 步綠、測試通道實測、Sol 驗收 1 輪 PASS。

# refactor-skipped-rules-visible — 重構套用略過的規則與觸發要讓玩家看得到

## Summary
playdata-loss-fixes 稽核發現（範圍外舊碼）：重構套用時，對不到狀態樹的規則與觸發由 `keep_rule`／`keep_trigger`（`refactor/apply.rs` 約 609 行）默默略過、只記 log，例如「第 7 天斥候登場」可能根本不生效。三題：玩家會遇到（會）、不做會怎樣（劇情事件不觸發且不知道）、跟地基有關（資料正確）。

併入 playdata-loss-fixes 的 U1：同欄多份規則矛盾時（`refactor/rule_merge.rs` `reconcile`）取 uid 最小那份只記 warn，也走同一個結果框顯示。

## 預算
中案，核心估約 120 行（後端收集略過與矛盾清單、接到結果框、i18n 不算）；測試 ≤ 核心 2 倍；動到結果框要跑測試通道。

## 做法（定案）
- `Candidates::collect`（apply_mechanism.rs）收 `SkippedMechanics { rules 路徑, triggers 標題, conflicts 路徑 }`，`reconcile` 回傳 `(規則, 是否真矛盾)`；`RefactorApplySummary.skipped` 帶到前端。
- 套用完成的結果框（`refactorApplyMessage`）在原摘要下各加一行：略過規則、略過觸發（劇情事件不會觸發）、矛盾只取一份的欄位；名稱最多列 5 個，有任一行就補「可以換較強的模型重新重構」。沒有略過就不多一行。

## 觀察
- 觸發被略過也可能是條件或旗標含 `*`，結果框「找不到對應的狀態欄位」措辭不完全準〔模型判斷·未裁決〕。

## 已知限制
- `apply.rs` 1001 行，列入下次體檢。
- 名稱原樣顯示模型自取的路徑與觸發標題，不一定好讀。
