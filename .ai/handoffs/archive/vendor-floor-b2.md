# vendor-floor-b2 — 額度「已省」每輪先扣掉 CLI／供應商自帶底線

## 現況
已結案合併進 main。定案與已知限制見 [plans/vendor-prefix-floor.md](../../plans/vendor-prefix-floor.md)「省額」「claude 通道不認底線」兩段。

- 認得出底線的組每輪先扣 min(讀到, 底線)，已省只算我方內容〔作者裁決 2026-10-28〕（codex 讀到 12,000＝我方讀到 992、省下等值 892.8）。
- claude 通道不認底線〔模型判斷·未裁決〕：實測與帳本顯示 claude 組的底線含我方固定 system。
- 正式帳本重放：頭條 55.4%→54.9%、codex 12.6 萬→5.4 萬、sonnet-5-5 91 萬→74 萬；原始用量與花費不變。Sol、opus-review 驗收 PASS。
