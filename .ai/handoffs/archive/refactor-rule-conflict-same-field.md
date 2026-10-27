# refactor-rule-conflict-same-field — 兩條機制條目寫同一欄規則，全部套用整次被擋（已結案）

規格與已知限制見 [plans/refactor-rule-conflict-same-field.md](../../plans/refactor-rule-conflict-same-field.md)。

## 結論
- 同一欄多份候選規則（跨條目、同條目不同字形）在 `refactor/apply_mechanism.rs` 的 `Candidates::collect` 整欄收齊後一次處理（`refactor/rule_merge.rs`）：相容合併、真矛盾原樣取來源 uid 最小那份並記 warn，不再整次拒套；字形歧義照舊拒套。
- 根因：吸收是並行呼叫、只共用盤點 FIELDS 的欄位名，選填的 min／max／inject 常寫得不一樣，舊判定整體 `!=` 就整次拒套。
- Sol、Grok 驗收 DONE；verify 12 步綠。

## 剩下
- 實機：clean-orc 真重構一鍵全部套用，排[實測佇列](../../reference/verification-queue.md)梯 2 第 25。
- 兩條目對同欄的觸發並存重複印：另案 refactor-trigger-overlap。
