# Task
Task-ID: release-2-ci-windows
Title: 發佈 2：CI 產線＋Windows 安裝檔（tauri-action）
Status: todo
Created: 2026-07-22T22:35:12.109145+08:00
Updated: 2026-07-22T22:35:12.109145+08:00

## Summary
tauri-action 單一 workflow 同時出 macOS（簽章＋公證）與 Windows（NSIS）產物，發到 GitHub release（NewPlan §16.3–16.4）。Windows 不買簽章憑證，接受 SmartScreen 警告；NSIS 設定 WebView2 缺少時自動安裝。

## Next action
- 等 release-1-mac-signing 的憑證就緒後開工：寫 tauri-action workflow，Developer ID .p12 與公證 API key 進 CI secrets；Windows 產物由協力者在乾淨 Windows 機驗收（下載→安裝→啟動，記錄 SmartScreen 實況）

## Constraints
單一 build，無免費／收費雙版本；自動更新 v1 不做，更新靠重新下載 release；發布帖須透明聲明資料流向（NewPlan §16.4）。
