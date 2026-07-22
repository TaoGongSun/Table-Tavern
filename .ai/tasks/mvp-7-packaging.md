# Task
Task-ID: mvp-7-packaging
Title: MVP 切片 7：打包 DMG＋README
Status: in-progress
Created: 2026-07-18T22:55:00.609136+08:00
Updated: 2026-07-20T01:59:44.191131+08:00

## Summary
tauri build 產出 DMG（ad-hoc 簽章）＋README 重寫，已提交（76664fd）。產物：Table Tavern_0.1.0_aarch64.dmg（4.2MB）；codesign=adhoc；open 啟動驗證通過。

## Next action
- 使用者以乾淨情境（另一台 Mac 或 AirDrop 補 quarantine）雙擊驗證 Gatekeeper 流程與 README 相符，通過後結案

## Constraints
ad-hoc 即可，不上架不公證；不做規避偵測。
