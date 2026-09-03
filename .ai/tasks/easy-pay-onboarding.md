# Task
Task-ID: easy-pay-onboarding
Title: 簡易付費入口：OAuth 一鍵連接 →（條件觸發）App 內儲值
Status: todo
Created: 2026-08-13T00:30:00.418457+08:00
Updated: 2026-09-03T17:53:09+08:00

## Summary
把新手接入從「註冊 OpenRouter → 建 key → 貼 key → 選模型」收斂成「連接 AI，開始遊戲」一顆鈕。遠期構想，等現行 BYOK 版（難以搞懂版）初步測試後才啟動。定位前提（2026-07-29 與 Sol 三方討論拍板）：Table Tavern 是**允許成熟題材的通用私人 RP 工具**，不以情色為商品——官網、預設內容、宣傳、付費點全部 SFW，模型推薦只標品質與價格。此定位是本任務所有金流與合規判斷的地基，動搖它整份計畫要重算。

規格細節（第一階段：OAuth 連接（先做）、第二階段：App 內儲值（條件觸發，非預設路線）、地區阻擋（僅第二階段需要，最低標準三件））見 [plans/easy-pay-onboarding.md](../plans/easy-pay-onboarding.md)。2026-09-03 的 AI 連線頁新方向見 [ai-connection-provider-panels](ai-connection-provider-panels.md)：所有 provider 同頁可見，OpenRouter OAuth 日後接入該面板並預設推薦免費模型。

## Next action
- 遠期構想；若開工，先與 `ai-connection-provider-panels` 對齊 OpenRouter panel，再做第一階段 OAuth。完整路線圖與合規前提見規格檔。

## Constraints
- BYOK／CLI 永不移除；2026-09-03 起不再整體收進「進階」摺疊，所有連線方式同頁可見，由各 provider 顯示自己的設定。內容政策分流（誰的帳號、誰負責）仍依賴 BYOK／CLI 路線存在。
- 第一階段不建任何新基礎設施；OAuth 換到的 key 沿用現有本機儲存與風險提示機制。
- OAuth 成功後應直接進入 OpenRouter 的簡化面板並使用推薦免費模型；高／中／低屬可展開的自訂模型分級，不是新手必填。
- 第二階段兩封書面核可缺一不收錢。
