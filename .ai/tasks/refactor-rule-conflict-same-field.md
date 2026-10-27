# refactor-rule-conflict-same-field — 兩條機制條目寫同一欄規則，全部套用整次被擋

## Summary
card-field-overlaps-fixed-keys 真重構實測（clean-orc、claude CLI Sonnet、characters 模式）：兩條機制條目各自為 `驻守天数` 寫了規則，「全部套用」回 `refactor rules conflict on 驻守天数` 整次失敗，玩家無法套用重構。

## 已知線索
- 產物存於該次實測（交接檔 [card-field-overlaps-fixed-keys](../handoffs/archive/card-field-overlaps-fixed-keys.md) 有經過）。
- 方向待查：合組時去重／合併同欄規則、或衝突時只擋該欄而非整次。

## Next action
未排程；先重現，查衝突判定的位置與兩條規則是否其實相同。

## 驗收方向
clean-orc 真重構可以全部套用；真正矛盾的規則仍有明確處理。
