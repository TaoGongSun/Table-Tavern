# API 請求送出後、回應頭之前沒有逾時

Status: 已結案（2026-10-11，三方驗收 PASS）

## 現況
- 方案定案：[plans/api-request-header-timeout.md](../../plans/api-request-header-timeout.md)（範圍、時限、包法、測試、已知限制）。
- 已進 main：桌面三處串流回應頭與首字共用 300 秒期限、錯誤本文 30 秒、生圖整段 300 秒、OAuth 15 秒、網頁版串流同步。
