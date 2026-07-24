# Task
Task-ID: test-build-cross-platform
Title: 測試版打包：Mac DMG（ad-hoc 簽章）＋Windows 安裝檔（CI 未簽章）
Status: todo
Created: 2026-07-24T15:00:00+08:00
Updated: 2026-07-24T15:00:00+08:00

## Summary
2026-07-24 與使用者拍板：出一版可獨立運行的測試版，Mac＋Windows 都要。Mac：DMG＋**ad-hoc 簽章**（機上 `security find-identity` 確認零憑證，使用者的 FVN Pirahus 同樣無憑證仍可簽後直跑，即 ad-hoc；正式 Developer ID＋公證仍歸 release-1）；使用者會拿 MacBook Air（另一台機）實測 Gatekeeper 實況——期望至少能「右鍵開啟／設定點信任」通過，不再出現「已損毀」（mvp-7-packaging 已修 linker-signed 問題，其交接檔有打包細節與踩坑紀錄，開工必讀）。Windows：repo 已在 GitHub，用 GitHub Actions＋tauri-action 出未簽章 .msi/.exe（SmartScreen 會警告，測試版可接受）；workflow 寫好即是 release-2 的地基。

## Next action
- 新對話開工：先讀 .ai/handoffs/mvp-7-packaging.md，確認現行 tauri build／DMG 流程與 ad-hoc 簽章現況 → Mac 出 DMG 交使用者拷去 MacBook Air 實測 → 同步寫 .github/workflows 的 tauri-action workflow 出 Windows 測試包

## Constraints
測試版不動 release-1（正式簽章公證）範圍；CI secrets 本輪不需要（未簽章）；DMG 需在乾淨路徑實測掛載開啟；Windows 產物請使用者或協力者在真 Windows 機驗收
