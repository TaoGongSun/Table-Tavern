# Task
Task-ID: release-1-mac-signing
Title: 發佈 1：Mac 正式簽章＋公證（Developer ID＋notarytool）
Status: todo
Created: 2026-07-22T22:34:38.537868+08:00
Updated: 2026-07-22T22:39:07.935825+08:00

## Summary
取代現行 ad-hoc 簽章：Developer ID 簽章＋notarytool 公證，讓付費使用者雙擊即開（NewPlan §16.2）。只出 Apple Silicon。AGPL-3.0 LICENSE 已落地（LICENSE 全文＋package.json／Cargo.toml 標 AGPL-3.0-only）。

## Next action
- 等使用者加入 Apple Developer Program（99 美元/年）後開工：設 Developer ID 憑證＋notarytool 公證流程，本機驗證通過後把憑證交接給 release-2 的 CI secrets

## 已鋪好的前置（mvp-7 2026-07-22）
- `src-tauri/tauri.conf.json` 已有 `bundle.macOS.signingIdentity: "-"`（正式 codesign 蓋 ad-hoc，含 hardened runtime、Info.plist 綁定、資源封裝）。憑證到手後把該值改為 `"Developer ID Application: <名稱> (<TEAMID>)"` 即可，其餘結構不用動。
- 驗收指令：`codesign -dvvv <app>` 看 flags／Info.plist／Sealed Resources；`spctl -a -vv <app>` 公證後應由 rejected 轉為 accepted。
- 重現下載情境測 Gatekeeper：`xattr -w com.apple.quarantine "0081;$(printf %x $(date +%s));Safari;" <dmg>`（測完 `xattr -d` 清掉），比 AirDrop 精準可重複。
- 公證通過後刪掉 README「第一次開啟被 Gatekeeper 擋下？」整段。

## Constraints
不上 App Store；公證走全自動 notarytool，可整合 CI；只出 aarch64，不做 Intel／universal（NewPlan §16.2）。
