# Task
Task-ID: quota-insufficient-alert
Title: 額度不足／AI 請求失敗提示：結構化分類＋攔截式彈窗（聊天＋生圖）
Status: todo
Created: 2026-07-27T20:30:00+08:00
Updated: 2026-07-27T20:30:00+08:00

## Summary
2026-07-27 與使用者討論定案：目前 AI 請求失敗時（額度用完、金鑰失效、速率限制等）只把原始狀態碼＋body 壓成字串，掛在頁尾一條容易被忽略的 `<p role="alert">`（聊天）或生圖對話框的 `aiGenError`，完全不分類、看不懂。要改成「攔截並跳出提示」。

現況重點（探索結論）：
- 錯誤**本來就不會進對話歷史**——串流殘句會被丟掉（`setStreamText("")`），不 append 任何角色訊息。此性質要保留。
- 後端只判 `is_success()`，失敗即 `format!("API 回應 {status}：{body}")`，無 402/429/401 分類，無 body 關鍵字解析。
- 聊天與生圖是兩條獨立錯誤路徑，兩邊都要接。
- CLI 訂閱供應商（Claude/Codex/agy/Grok）只拿得到一串文字，無狀態碼，只能關鍵字 heuristic。

定案方案：
- **後端回結構化錯誤** `{ kind, status, providerMessage }`，kind 分五類：`quota / auth / rate_limit / network / unknown`。
  - `transport.rs` 兩個 chokepoint（chat 419-422、image 467-474）加狀態碼＋body 關鍵字判斷。
  - `cli.rs`（~602）對 `text` 做關鍵字 heuristic；判到額度相關就歸 `quota`。
- **前端單一資訊性 modal**，按 kind 換文案，只有「關閉」鈕（不做重試按鈕）。
- **送出失敗保留輸入框內容**：對所有 kind 一致，關掉 modal 後可直接再點送出重送同一句。GM 旁白／推進／生圖是按鈕觸發、無輸入框內容，關掉後再點該按鈕即可，不特別處理。
- 生圖沿用其對話框（本就在 modal 內），額度不足時用同套分類文案，不再疊一層。
- 串流殘句一律丟棄不進歷史（維持現況）。

文案（zh-TW，en 同步）：
| kind | 標題 | 內文方向 |
|---|---|---|
| quota | 額度可能不足 | 請確認你的 AI 供應商額度（**不提加值、不暗示加值可解決**；CLI 訂閱無法加值或很貴，只請使用者自行確認） |
| auth | 金鑰可能失效 | 請到設定確認 AI 連線金鑰 |
| rate_limit | 請求過於頻繁 | 稍候再試（無重試鈕，訊息已保留可直接再送） |
| network / unknown | AI 請求失敗 | 附原始錯誤訊息供排查 |

## Next action
- 開工先定 kind 判斷準則：各 status code 對應（402→quota、401/403→auth、429→rate_limit）＋ body/CLI 文字關鍵字清單（如 insufficient/quota/balance/餘額 → quota）。
- 後端：改 `transport.rs` 兩處與 `cli.rs` 回結構化錯誤（需確認跨 Tauri command 邊界如何序列化，目前全走 `.to_string()`，要改成可帶 kind 的型別）。
- 前端：做共用 error modal 元件，接聊天（`App.tsx` requestReply/gmNarrate/gmAdvance/advance_scene 的 catch）與生圖（`generateImage` catch / 生圖對話框）；改送出流程為失敗保留輸入內容。
- i18n：`src/i18n.ts` 補雙語文案。
- 驗證：cargo test 綠＋npm build 綠；分類正確性請使用者用真實用完額度的金鑰／故意打錯金鑰實測（自動測難模擬真實 402）。

## Constraints
- 這是 BYOK：額度是使用者自己供應商帳戶的餘額，文案語氣須為「請你確認」而非「我們的服務」，且不引導加值。
- CLI 訂閱只能關鍵字盡力猜，文案用「可能」留餘地，勿假裝準確。
- 只改錯誤呈現與分類，不改任何 AI 請求成功路徑；串流殘句不進歷史的既有行為不可回退。
- 保留輸入框內容只對「使用者打字送出角色回話」那條有效；按鈕觸發的動作不需另做。
