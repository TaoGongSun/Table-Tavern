# API 串流停滯逾時（已結案）

已進 main。做法與定案見 [../../plans/api-stream-stall-timeout.md](../plans/api-stream-stall-timeout.md)；verify 10 步綠，Sol、Grok 驗收通過。

## 已知與未驗
- GUI 端到端與 ErrorNote／TurnFailedDialog 實際渲染未驗；未對真 OpenRouter 驗。
- 只送 `reasoning_details` 陣列、沒有 reasoning 字串的供應商不算進展，首字窗後可能被判停滯〔模型判斷·未裁決〕。
- 網路 chunk 錯誤直接回、不經記帳（既有行為）。
- `send()` 到回應頭前的無限等待不在本案，由主線另立案。
