# api-request-header-timeout 做法（已結案 2026-10-11，三方驗收 PASS）

## 一、範圍

| 呼叫點 | 處理 |
|---|---|
| `transport/client.rs` `stream_chat`（含 Claude 相容端點）、`stream_chat_models`（智慧免費）、`transport/responses.rs` | 回應頭與首字共用同一期限；非 2xx 錯誤本文 30 秒 |
| `transport/client.rs` `generate_image` | `/images` 送出＋讀本文（成功與失敗）整段 300 秒 |
| `openrouter_oauth.rs` `exchange_code` | 送出＋讀本文整段 15 秒，與網頁版一致 |
| 網頁版 `web/.../stream-chat.ts` | 同桌面版 |

排除〔模型判斷·未裁決〕：前端模型清單、回退點下載、更新外掛的 check／download（非 AI API，另案再說）。已有整段時限而不動的：`key_tier`（5 秒）、`smart_free/api.rs`（15／20 秒）、`generated_image::download`、網頁版 `openrouter-api.ts`（15 秒）。

## 二、時限

- **所有路線（含自訂網址／本地伺服器）一律 5 分鐘，不分流**〔作者裁決 2026-10-11〕：`StallGuard` 在送出前建好，`timeout_at(首字期限, send())` 包回應頭，回應頭到了首字沿用同一期限，所以送出到首個合格進展的總預算就是 300 秒；逾時碼照舊 `AI_STREAM_STALLED: idle_secs=300`。OpenRouter 串流的回應頭通常即時，取決於上游。
- **非 2xx 錯誤本文 30 秒**：逾時或讀失敗都當空本文，狀態碼與 rate-limit 標頭照報，不新增錯誤碼。
- 生圖、OAuth 的 client 建在時限外（載憑證可能數秒）；生圖 402／403 後查 key 層級也在時限外（有自己的 5 秒）。
- 不開玩家設定。

## 三、包法

- `transport/stall.rs`：`StallWindow` 加 `error_body`；`StallGuard::send`（回 `Sent::Response｜Failed｜Stalled`）與 `StallGuard::error_body`。三處串流把 `StallGuard::new` 移到送出前。
- 生圖：`image_reply` 內部函式整段包 `tokio::time::timeout`，逾時回 `image request timed out after 300s`（不掛碼，走生圖既有未知錯誤文案）。
- OAuth：拆出吃 URL 的 `exchange_code_at`，逾時回既有 `ERR_NETWORK`。
- 網頁版：送出時起計時器（到首字期限，`deadline - Date.now()`），到點設旗標並 `local.abort()`；fetch 的 `finally` 清計時器（不清會誤殺長串流）；catch 先看逾時旗標回 `failureStalled`、再看外部取消回 aborted、最後 `failureNetwork`。錯誤本文 `Promise.race` 逾時就 `local.abort()` 回空本文，所有出口清計時器。`StallWindow` 加 `errorBodyMs`。

## 四、測試

- Rust（`client/timeout_tests.rs`、`responses.rs`、`openrouter_oauth.rs`）：`test_support::hang_server`（讀一次請求後只寫指定位元組——空、或回應頭加半截本文——撐 30 秒才關）、`stall_server_after`（回應頭延遲）＋ `watchdog`（10 秒）。三處串流各「回應頭不來→停滯碼」「錯誤本文卡住→狀態碼照報」（智慧免費另驗 Timeout 分類、狀態與 rate-limit 標頭保留）；回應頭晚到＋首個進展晚到、各自小於首字窗合計超過→必須逾時；回應頭已到、持續進展且總時間超過首字窗仍成功；生圖回應頭不來／200 後成功本文卡住；OAuth 兩種卡法都回 `ERR_NETWORK`。三處串流的卡住測試換回舊碼會紅（watchdog），「回應頭到後重建期限」的回歸會讓共用期限測試紅。
- 網頁版 vitest：回應頭不來→停滯；等回應頭時按停止→aborted；錯誤本文卡住→狀態與 rate-limit 標頭保留、本文空、傳入 fetch 的 signal 已中止；回應頭晚到＋首字晚到合計超過→逾時；慢串流超過首字窗不誤殺（驗計時器有清）。拿掉 `local.abort()` 或回應頭後重設期限都會紅。

## 五、已知限制

- Ollama／llama.cpp 等本地伺服器要等首字才回應頭，慢電腦＋長提示詞超過 5 分鐘會被判逾時；超過 5 分鐘一輪玩家早就不耐煩，不是程式該等的，本地模型慢到這程度本來就不適合拿來玩〔作者裁決 2026-10-11〕。
- 逾時丟掉 future＝取消本地等待並斷線，不保證上游停止或不計費。
- 玩家按停止與逾時同時就緒時可能回逾時而非取消（網頁版 catch 先看逾時旗標）。
- 網頁版回應頭逾時不帶額外尾巴，與停滯同碼。
