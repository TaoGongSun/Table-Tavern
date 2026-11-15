# release-9 — v0.5.4 發佈

Status: v0.5.4 已公開（2026-11-15）。
流程沿用 [plans/release-2-ci-windows.md](../plans/release-2-ci-windows.md)「發版檢查清單」。版號 0.5.4〔作者裁決 2026-11-15〕。

## 已在 main
- 五處版本號 0.5.4（`node scripts/check-version.mjs v0.5.4` 綠）、`npm run verify` 18 步通過。
- CHANGELOG `[0.5.4] — 2026-11-15`（繁中＋英文；主線 Fable 直接寫，白話審查即主線）。真正發佈日不是這天的話，發之前改日期。
- README 兩語：跑團段改成五本內建規則書、「先攻」改「行動順序」；授權段補四本新書的來源與授權。
- 手機端 `party/` 十語系與房間說明書三語的「先攻」改成「行動順序」。
- 截圖重拍 6 組 × 三語：下載頁 `status`、`vnWizard`，說明書 `room-13`／`15`／`17`／`19`（主線親看）。圖上頂列版號是 v0.5.3（拍照時還沒改版號），照上一版慣例不重拍〔模型判斷·未裁決〕。`room-06`／`08`／`10` 桌面標頭格式沒變，不重拍〔模型判斷·未裁決〕。
- 拍照手法與腳本在 `TestCards/download-tour/SHOOTING.md`（本機限定，已補這次的修正與「怎麼接生圖來源」）。

## 已做（2026-11-15，作者說「部署並演練打包」）
- 手機端已部署：`tt-room.pirahus.com`，Version 9e7b2ec5；線上檔案已確認是新版。伺服器邏輯沒改。
- 演練 `test-v0.5.4`：check、build-macos、build-windows、finalize 全綠，artifact 8 個檔齊，`latest.json` 版本 0.5.4、兩平台都在。演練 tag 還留著，正式發佈後刪。

## 已發佈（2026-11-15）
- `v0.5.4` 打在演練過的 5eb761d2；CI 全綠；release 頁 8 個檔齊、標 Latest、非草稿；`releases/latest/download/` 的 `latest.json`（0.5.4、兩平台）、`TableTavern-Windows-Setup.exe`、`TableTavern-macOS.dmg` 都下載得到。
- 演練 tag `test-v0.5.4` 已刪。兩個帶版號的安裝檔已放 `~/Downloads/TableTavern-v0.5.4/` 交作者傳 pCloud。
- 沒核對的：網頁版下載頁兩顆鈕實際指向的檔名（它執行時向 GitHub 查最新版）。
- 剩作者本人才驗得到的都在待實測清單：兩平台下載安裝、0.5.3→0.5.4 的 App 內更新與回退、要新版才出現的對話窗。

> 結案（2026-11-15）：v0.5.4 已公開。
