# Task
Task-ID: easy-pay-onboarding
Title: 簡易付費入口：OAuth 一鍵連接 →（條件觸發）App 內儲值
Status: todo
Created: 2026-08-13T00:30:00.418457+08:00
Updated: 2026-08-13T00:30:00.418457+08:00

## Summary
把新手接入從「註冊 OpenRouter → 建 key → 貼 key → 選模型」收斂成「連接 AI，開始遊戲」一顆鈕。遠期構想，等現行 BYOK 版（難以搞懂版）初步測試後才啟動。定位前提（2026-07-29 與 Sol 三方討論拍板）：Table Tavern 是**允許成熟題材的通用私人 RP 工具**，不以情色為商品——官網、預設內容、宣傳、付費點全部 SFW，模型推薦只標品質與價格。此定位是本任務所有金流與合規判斷的地基，動搖它整份計畫要重算。

規格細節（第一階段：OAuth 連接（先做）、第二階段：App 內儲值（條件觸發，非預設路線）、地區阻擋（僅第二階段需要，最低標準三件））見 [plans/easy-pay-onboarding.md](../plans/easy-pay-onboarding.md)。

## Next action
- 遠期構想，等 BYOK 版初步測試後先做第一階段 OAuth；完整路線圖與合規前提見任務檔

## Constraints
- BYOK／CLI 永不移除，收進「進階」摺疊——內容政策分流（誰的帳號、誰負責）依賴它。
- 第一階段不建任何新基礎設施；OAuth 換到的 key 沿用現有本機儲存與風險提示機制。
- 第二階段兩封書面核可缺一不收錢。
