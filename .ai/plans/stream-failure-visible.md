# stream-failure-visible — 規格

實測起點：2026-08-21 使用者以 OpenRouter 免費 `deepseek/deepseek-v4-pro-0813-free` 連打，5 次呼叫中 2 次「思考完就停」——串流正常走完 `[DONE]`、usage 尾塊有回，但 `delta.content` 零字。空回應被當正常 GM 回合寫進 transcript、進入下一次呼叫的歷史，直接造成第 3 次答非所問。全機 950 則 transcript 事件中 3 則空白事件全出自這天。

## 拍板結論（2026-08-21，與 Sol 三輪收斂）

**四段防守，缺一不可**
1. `transport::stream_chat`：SSE 的 `error` 塊原樣拋出；收工時判 `trim()` 後正文為空、判異常 `finish_reason`。
2. `gm_narrate`：`extract_state_block`／`extract_next_speaker` 剝殼後再判一次空，**擋在 `apply_block` 之前**——否則失敗回合照樣重擲一輪骰。
3. 通過語意檢查才准寫 state、記 mechanism log、處理登場。
4. 前端 `appendEvent` 拒收空白事件（資料污染保險）。

**判定優先序**（固定，`length`＋空正文歸 incomplete 不歸 empty）
SSE error 原話 → `content_filter` → `length`／未知終止原因／無 `[DONE]` 就 EOF → 正常收尾但正文 trim 後空 → 成功

**三個錯誤碼**（錯誤字串前綴，不動 `DataResult<String>`）
- `AI_EMPTY_RESPONSE`：正常收尾但可見正文為空（含 GM 剝殼後 `display` 為空；前端保險層觸發時同碼）
- `AI_INCOMPLETE_RESPONSE`：`finish_reason` 為 `length`／`tool_calls`／未知，或無正常收尾就 EOF
- `AI_CONTENT_FILTERED`：`finish_reason` 為 `content_filter`
SSE `error` 塊**不加碼**，原樣 `Err(error.message)`——免費層 429 的原話能被既有 `QUOTA_ERROR` 正則接住，玩家看到「額度用完」而非籠統錯誤。`error.message` 缺失或非字串時序列化整個 `error` 物件，不得靜默忽略。

`explainAiError` 先 `startsWith` 認碼，認不到才退回既有 quota／auth 正則；既有分流不動。

**不做**：自動重試；拒絕偵測（2026-08-21 使用者裁決：那句簡體拒絕是刻意測 NSFW 的預期結果）；`delta.reasoning` 心跳（獨立小改動，另案）。

**不改回傳型別**：實測失敗全為零正文，「半截正文」尚未發生；真出現再擴成 `{ text, finish_reason, complete }`。reasoning token 只供錯誤診斷，走 API 路徑獨立 `Option<u64>`，不擴充 `PromptCacheUsage`（避免連帶改 CLI parser 與測試資料）。

## 驗收
- `cargo test`／`vitest`／`npm run build`／`npm run check:i18n` 全綠
- 新增 SSE 案例：中途 error 塊、`finish_reason=length`、`content_filter`、正常收尾但零內容、無 `[DONE]` 就 EOF
- 十語系各補 3 條文案
