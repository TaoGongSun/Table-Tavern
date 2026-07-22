# Task handoff
Task-ID: mvp-7-packaging
Updated: 2026-07-19T17:59:31.963479+00:00
Status: in-progress

## Goal
完成 MVP 切片 7「打包」：tauri build 產出 macOS DMG（ad-hoc 簽章，不上架不公證）＋重寫 README（安裝、Gatekeeper、BYOK 與 CLI 模式、資料位置），並在乾淨環境驗證雙擊可開（KICKOFF §1／§5.7）。

## Current state
build 與 README 完成並提交（76664fd）；產物驗證通過，只剩「乾淨環境雙擊＋Gatekeeper 流程」需使用者在未去除 quarantine 的情境走一次。

## Completed
- npm run tauri build 成功：src-tauri/target/release/bundle/macos/Table Tavern.app＋dmg/Table Tavern_0.1.0_aarch64.dmg（4.2MB）
- codesign -dv 確認 Signature=adhoc（linker-signed）
- open 啟動 .app：程序存活 6 秒無崩潰，AppleScript quit 正常關閉
- README.md 整份重寫（安裝、Gatekeeper 右鍵開啟／xattr、BYOK、CLI 風險一句、資料位置、開發指令），提交 76664fd

## Verification
- 建置 log：scratchpad tauri-build.log 末段「Finished 2 bundles」
- codesign 輸出 Signature=adhoc
- pgrep 證實程序啟動、quit 後消失
- 未驗：乾淨環境（帶 quarantine 屬性）雙擊與 Gatekeeper「仍要打開」流程——本機建置產物無 quarantine，無法自動重現

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main
- HEAD: 76664fdb0c66e39e9cf59b8b9c46205e90b4a7e2
- Dirty: false
- Dirty fingerprint: 4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945

## Remaining
- 使用者把 DMG 傳到另一台 Mac（或 AirDrop 給自己讓系統補 quarantine）雙擊驗證 Gatekeeper 流程與 README 說明相符，通過後結案

## Next action
- 使用者以乾淨情境雙擊 DMG 內 App，照 README 的 Gatekeeper 步驟走一次

## Constraints
- ad-hoc 簽章即可，不上架、不公證（KICKOFF §1）
- 不做規避偵測；README 如實描述 CLI 模式風險（NewPlan §4.3）
