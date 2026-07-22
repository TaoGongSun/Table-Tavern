# Task handoff
Task-ID: mvp-7-packaging
Updated: 2026-07-22T15:55:00+00:00
Status: completed

## Goal
完成 MVP 切片 7「打包」：tauri build 產出 macOS DMG（ad-hoc 簽章，不上架不公證）＋重寫 README（安裝、Gatekeeper、BYOK 與 CLI 模式、資料位置），並在乾淨環境驗證雙擊可開（KICKOFF §1／§5.7）。

## Current state
結案。2026-07-22 以手動貼 quarantine 重現下載情境實測，抓到並修掉致命缺陷（linker-signed 簽章被判「已損毀」），Gatekeeper 現在走正常的未公證流程。最後一哩（系統設定按「仍要打開」）刻意不驗——使用者已決定購買 Apple Developer Program，該流程將由 release-1-mac-signing 的公證方案取代。

## Completed
- npm run tauri build 成功：src-tauri/target/release/bundle/macos/Table Tavern.app＋dmg/Table Tavern_0.1.0_aarch64.dmg（4.2MB）
- codesign -dv 確認 Signature=adhoc（linker-signed）
- open 啟動 .app：程序存活 6 秒無崩潰，AppleScript quit 正常關閉
- README.md 整份重寫（安裝、Gatekeeper 右鍵開啟／xattr、BYOK、CLI 風險一句、資料位置、開發指令），提交 76664fd
- 2026-07-22 修正（未提交）：tauri.conf.json 加 bundle.macOS.signingIdentity="-"，讓 Tauri 實際呼叫 codesign 蓋 ad-hoc；否則只有連結器的 linker-signed 簽章，Info.plist 與資源都不在簽章範圍，帶 quarantine 時 Gatekeeper 直接判「已損毀，應丟到垃圾桶」（無任何逃生口）
- 2026-07-22 README 修正：Gatekeeper 段落改為主推「系統設定 → 隱私權與安全性 → 仍要打開」，刪掉已失效的「右鍵開啟」；對話框文案更新為「Apple 無法驗證是否為惡意軟體」

## Verification
- 建置 log：scratchpad tauri-build.log 末段「Finished 2 bundles」
- codesign 輸出 Signature=adhoc
- pgrep 證實程序啟動、quit 後消失
- 2026-07-22 Gatekeeper 實測（手動 `xattr -w com.apple.quarantine "0081;<hex>;Safari;"` 貼在 DMG 上重現下載情境，比 AirDrop 精準且可重複；測畢已清除）：
  - 修正前（linker-signed）：雙擊與右鍵開啟皆顯示「『Table Tavern』已損毀，無法打開。你應該將其丟到垃圾桶」，只有「取消／丟到垃圾桶」，無逃生口
  - 修正後（正式 ad-hoc）：改顯示「Apple 無法驗證『Table Tavern』是否為惡意軟體」＋「完成／丟到垃圾桶」，即正常的未公證流程
  - 簽章前後對比：`flags` 0x20002(adhoc,linker-signed) → 0x10002(adhoc,runtime)；`Info.plist` not bound → entries=14；`Sealed Resources` none → version=2 rules=13
  - `codesign --verify --deep --strict`：valid on disk、satisfies its Designated Requirement
  - `spctl -a -vv`：rejected（預期——ad-hoc 未經公證本就該被拒，但理由是未公證而非損毀）
  - macOS 26 實測確認：右鍵「打開」已不再提供例外選項，逃生口只剩系統設定
- 刻意未驗：系統設定按「仍要打開」後的啟動——使用者決定改買 Developer Program，此流程即將被公證取代

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main
- HEAD: 76664fdb0c66e39e9cf59b8b9c46205e90b4a7e2
- Dirty: false
- Dirty fingerprint: 4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945

## Remaining
- 無（本切片）。後續由 release-1-mac-signing 承接。

## Next action
- 無，已結案。移交 release-1-mac-signing：憑證到手後把 tauri.conf.json 的 `bundle.macOS.signingIdentity` 由 `"-"` 改為 `"Developer ID Application: <名稱> (<TEAMID>)"`，加 notarytool 公證，並刪掉 README 的整段 Gatekeeper 說明（公證後不再出現）

## Constraints
- ad-hoc 簽章即可，不上架、不公證（KICKOFF §1）
- 不做規避偵測；README 如實描述 CLI 模式風險（NewPlan §4.3）
