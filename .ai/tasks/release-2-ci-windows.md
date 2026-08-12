# Task
Task-ID: release-2-ci-windows
Title: 發佈 2：CI 產線＋Windows 安裝檔（tauri-action）
Status: todo
Created: 2026-08-13T00:30:01.405988+08:00
Updated: 2026-08-13T00:30:01.405988+08:00

## Summary
tauri-action 單一 workflow 出 macOS 與 Windows 產物，發到 GitHub release（NewPlan §16.3–16.4）。2026-07-24 使用者拍板：Windows 玩家基數大於 Mac、不再等 release-1，但簽章憑證可能很貴——**先發未簽章版，觀察玩家對 SmartScreen 警告的實際接受度再決定買不買**（次文化圈對「無簽章」未必反感，心理因素影響傳播）；候選便宜路線留檔備查：Certum 開源憑證、Azure Trusted Signing（價格與資格皆未查證，禁當事實引用）。NSIS 設定 WebView2 缺少時自動安裝。

## Next action
- 出未簽章版＋發布說明附 SmartScreen 繞過步驟，先觀察玩家接受度再拍板買簽章（2026-07-24 拍板）

## Constraints
單一 build，無免費／收費雙版本；自動更新 v1 不做，更新靠重新下載 release；發布帖須透明聲明資料流向（NewPlan §16.4）。
