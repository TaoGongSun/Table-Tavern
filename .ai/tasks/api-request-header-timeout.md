# API 請求送出後、回應頭之前沒有逾時

Status: todo

## Summary
api-stream-stall-timeout 只管 200 之後的串流停滯。`send()` 到回應頭之前，以及非 2xx 時讀 `response.text()`，仍會無限等：三處 API 呼叫用 `reqwest::Client::new()`，reqwest 0.12 預設沒有 timeout。不能用 `ClientBuilder::timeout` 補——它的時限含整段 body，正常的長串流會被殺。2026-10-07 api-stream-stall-timeout 送審時 Sol、Grok 一致指出，決定另案。

## Next action
只包 `send()` 的回應頭等待與非 2xx 的錯誤本文讀取（各自限時），錯誤沿用 api-stream-stall-timeout 的人話路線或既有網路錯誤碼；補假伺服器測試（接受連線但不回應頭）。
