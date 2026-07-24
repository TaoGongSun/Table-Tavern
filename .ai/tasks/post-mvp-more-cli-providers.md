# Task
Task-ID: post-mvp-more-cli-providers
Title: MVP 後：擴充 CLI 訂閱供應商（本輪：gemini 端到端＋一鍵安裝；grok 留偵測介面）
Status: in-progress
Created: 2026-07-20T09:48:42.461107+08:00
Updated: 2026-07-25T00:20:00+08:00

## Summary
2026-07-25 與使用者拍板（取代 07-20「只偵測不代辦」舊約）：本輪只做 gemini 端到端，grok 暫無額度、留偵測介面之後補。目標 CLI 是**官方 gemini-cli**（brew／npm 有正式通道；使用者機上的 `agy` 實為 Antigravity CLI，非本案目標）。新增「一鍵安裝」UX：app 開**可見終端機視窗**跑安裝腳本——開頭印「正在自動安裝 Gemini CLI，請勿關閉此視窗」→ 裝 CLI → 預寫設定跳過首跑互動 → 觸發官方 OAuth（瀏覽器登 Google、回跳到 CLI 不經 app）→ 腳本原地輪詢憑證檔，出現後印「驗證成功，已連結，可以關閉終端機視窗」收尾；app 同步輪詢更新自己的連線狀態。使用者除了在官方頁登帳號外零輸入。可見終端機＋CLI 端回跳＝降低第三方代辦觀感；app 全程不碰帳密與 token；風險告知勾選照舊前置。接入件同 claude／codex 模式：detect_clis 偵測、headless 單發參數組裝、逐行串流解析、檔位模型清單、tier_models 前綴鍵沿用。

## Next action
- 研究已派（安裝通道、首跑互動跳過、憑證檔訊號、headless 參數與模型清單）；回來後拍板腳本與接入規格再派工。測試機已備妥：agy 備份為 ~/.local/bin/agy.bak、brew gemini-cli 已移除，機上憑證 ~/.gemini/oauth_creds.json 保留（測全新登入流程時暫時搬走即可）。

## Constraints
app 不碰帳密／token（OAuth 全程官方頁面＋CLI 自持）；安裝過程必須可見（終端機視窗）；上下文一律 App 端組裝、不依賴 CLI session（NewPlan §8.1）；模型 id 不寫死；風險告知前置。
