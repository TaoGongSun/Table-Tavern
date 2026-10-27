# refactor-trigger-overlap — 多條機制條目對同一欄的觸發並存，同回合可能重複命中

## Summary
refactor-rule-conflict-same-field 查證時發現：衝突檢查只看規則不看觸發。clean-orc 兩條機制條目對 `驻守天数` 各自產出觸發（7–9／10–11／12–13／14+ 天 vs 7／14／21+ 天），套用後原樣並存，同一回合可能兩段都命中、同類事件提示印兩次。

## 已知線索
- 出處：[refactor-rule-conflict-same-field 交接檔](../handoffs/archive/refactor-rule-conflict-same-field.md)。
- 兩條目原文本來就各寫一套事件，未必算錯；要先看重複命中對 GM 實際的影響再決定去重或合併。

## Next action
未排程；先用 clean-orc 重現同回合雙命中，看 GM 提示實際長相。

## 驗收方向
同一欄的觸發不會讓 GM 同回合收到兩份矛盾或重複的事件提示。
