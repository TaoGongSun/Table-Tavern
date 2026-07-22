# Task
Task-ID: release-1-mac-signing
Title: 發佈 1：Mac 正式簽章＋公證（Developer ID＋notarytool）
Status: todo
Created: 2026-07-22T22:34:38.537868+08:00
Updated: 2026-07-22T22:34:38.537868+08:00

## Summary
取代現行 ad-hoc 簽章：Developer ID 簽章＋notarytool 公證，讓付費使用者雙擊即開（NewPlan §16.2）。只出 Apple Silicon。順帶落地授權拍板：repo 加 AGPL-3.0 LICENSE 檔。

## Next action
- 先做不用等的：repo 加 AGPL-3.0 LICENSE（§16 拍板）。簽章部分等使用者加入 Apple Developer Program（99 美元/年）後，設 Developer ID 憑證＋notarytool 公證流程，本機驗證通過後把憑證交接給 release-2 的 CI secrets

## Constraints
不上 App Store；公證走全自動 notarytool，可整合 CI；只出 aarch64，不做 Intel／universal（NewPlan §16.2）。
