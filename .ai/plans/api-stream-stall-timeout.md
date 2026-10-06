# api-stream-stall-timeout 做法（三方共識定案）

1. **層級**：三處 `bytes_stream()` 讀取迴圈（`transport/client.rs` 的 `stream_chat`、`stream_chat_models`，`transport/responses.rs`）共用 `transport/stall.rs` 的 `StallGuard`；對外函式不變，內部 `*_windowed` 接可注入的 `StallWindow`（測試用毫秒）。
2. **deadline 跨 chunk 保存**（`timeout_at`），只有「合格進展」才延後：chat 的 `delta.content`／`reasoning`／`reasoning_content` 非空、Responses 的 `output_text.delta` 或 `response.reasoning*.delta` 非空。保活註解、空行、半行、空 `data:`、壞 JSON、role-only 塊、`response.created` 都不續命，也不把首字窗切成進展窗。`emitted_text` 仍只看正文。
3. **窗口**：200 之後首個進展前 300 秒（推理模型首字慢），之後進展間隔 120 秒。不開玩家設定。〔三方共識，主線轉達〕
4. **收法**：記錄逾時、跳出迴圈、丟掉 stream（斷線，與玩家停止同機制）、**先記已收 usage**、再回 `AI_STREAM_STALLED: idle_secs=N`，收尾判定不覆蓋它。智慧免費路回 `ApiFailure::stalled`（`FailureStage::Timeout`，分類與連兩次換模規則不變）。
5. **接線**：`dispatch::ai_call_failure` 白名單加碼；`smart_free/call.rs display()` 遇停滯碼不包 `AI_FREE_MODEL_BUSY`；前端 `ai-error.ts` 的 `FAILURE_CODES` 加 `errStreamStalled`，十語系文案（「太久沒收到模型輸出，已中止這次等待」）。
6. **CLI 的 120 秒保持自己的常數**（數輸出行；API 數模型進展），兩邊註解互相指向。
7. **不處理**：`send()` 到回應頭前的無限等待（`ClientBuilder::timeout` 會殺正常長串流），另立案。
8. **測試**：本機假串流伺服器（`test_support::stall_server`）＋注入短窗口；判定靠錯誤碼不量時間（`Client::new()` 載憑證就數秒）。
