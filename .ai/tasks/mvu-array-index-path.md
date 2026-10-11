# 未重構 MVU 卡陣列索引路徑

Status: todo

## Summary
未重構 MVU 卡的陣列在本地狀態樹是一整片文字，上游合法的索引路徑（`/HP/0`、`/清單/2`）本地找不到而被拒收。照酒館行為補上：TypedView 與樹投影支援陣列索引的 replace／delta／insert／remove。`[值, 說明]` 的 delta 已併入 mechanism-error-feedback，本案只做索引路徑。

## Next action
未排程；mechanism-error-feedback 結案後再排。
