# quota-insufficient-alert — AI 請求失敗：攔截式彈窗＋送出失敗保留輸入

## Summary
2026-07-27 討論定案的三樣，錯誤分類已由 ai-error-messages 做掉（後端錯誤字串掛穩定前綴 `AI_HTTP_STATUS_nnn:`／`AI_CALL_FAILED:` 等，前端 [ai-error.ts](../../src/shared/ui/ai-error.ts) `explainAiError` 依狀態碼與 CLI 原話分類成人話文案）。剩兩樣：

1. **攔截式彈窗**：聊天失敗現在是頁尾 `ErrorNote`、生圖是對話框內小區塊，容易漏看。改單一資訊性 modal，只有「關閉」，文案沿用 `explainAiError`。生圖本來就在對話框內，用同套文案、不再疊一層。
2. **送出失敗保留輸入**：[useChatController.ts](../../src/features/play/useChatController.ts) 送出前就清空輸入框、玩家那句也先寫進逐字稿，失敗後找不回來。改成失敗時還原輸入框，且不留下沒有回覆的玩家句（或明確定義留下時的行為）。只管打字送出那條，按鈕觸發的動作不處理。

## Constraints
- BYOK：文案語氣是「請你確認你的供應商額度」，不引導加值。
- CLI 只能從原話猜，文案用「可能」。
- 串流殘句不進歷史的既有行為不可回退；只改錯誤呈現，不動成功路徑。
