# Task
Task-ID: mvp-7-packaging
Title: MVP 切片 7：打包 DMG＋README
Status: completed
Created: 2026-07-18T22:55:00.609136+08:00
Updated: 2026-07-22T23:55:00+08:00

## Summary
tauri build 產出 DMG（ad-hoc 簽章）＋README 重寫，已提交（76664fd）。2026-07-22 以手動貼 quarantine 實測 Gatekeeper，抓到 linker-signed 簽章被判「已損毀」的致命缺陷，加 tauri.conf.json 的 signingIdentity="-" 修正為正常的未公證流程；README 的 Gatekeeper 步驟同步改為系統設定路徑（macOS 15 後右鍵開啟已失效）。最後一哩不驗，改由 release-1-mac-signing 的公證方案取代。

## Next action
- 無，已結案。移交 release-1-mac-signing。

## Constraints
ad-hoc 即可，不上架不公證；不做規避偵測。
