# release-2-ci-windows — 桌面版首個正式版 0.3.0（Windows＋Mac，未簽章）

分支：已併入 main 並刪除（2026-10-12）。

## 現況
- 發版準備已進 main：五處版本 0.3.0、CHANGELOG `[0.3.0]`（繁中＋英文發布說明，日期 2026-10-12）、發版產物加固定檔名下載副本 `TableTavern-Windows-Setup.exe`／`TableTavern-macOS.dmg`。
- 兩平台首發都不簽章：Windows 先觀察玩家對 SmartScreen 的接受度再決定買不買憑證（2026-07-24 拍板）；Mac 同樣不簽〔作者裁決 2026-10-04〕。候選憑證路線（Certum 開源憑證、Azure Trusted Signing）價格與資格皆未查證，禁當事實引用。
- 首發不等端對端驗收，App 內更新的端對端改在第二版發出時驗〔作者裁決 2026-10-12〕。
- 發版流程、檢查清單、已知限制在 [plans/release-2-ci-windows.md](../plans/release-2-ci-windows.md)。

## 下一步
- 等主線推 tag 發版（`test-v*` 演練、`v0.3.0` 都由主線推），再照計畫「發之後」清單核對。
