# release-9 — v0.5.4 發佈

Status: 發佈前的準備都已進 main（版本 0.5.4、CHANGELOG、README、截圖）；手機端已部署、演練打包全綠；推正式 tag 等作者說。
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

## 等作者說才做
- 推 `v0.5.4` 正式 tag（tag 要指在 origin/main 上的 commit）。

## 發之後
- 照檢查清單核對 release 頁（8 檔、Latest、三個固定下載連結）、下載頁兩顆鈕指向新檔、刪演練 tag。
- 兩個帶版號的安裝檔放 `~/Downloads/TableTavern-v0.5.4/` 交作者傳 pCloud。
- 待實測清單裡要新版才驗得到的列：desktop-update-detect 兩版端對端、ui-redesign 的新版對話窗（含這版改的四語更新／重試鈕）、release-2 的兩平台安裝。
