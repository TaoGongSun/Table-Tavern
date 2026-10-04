# Task
Task-ID: release-1-mac-signing
Title: 發佈 1：Mac 正式簽章＋公證（Developer ID＋notarytool）
Status: todo
Created: 2026-08-13T00:30:01.306752+08:00
Updated: 2026-08-13T00:30:01.306752+08:00

## Summary
取代現行 ad-hoc 簽章：Developer ID 簽章＋notarytool 公證，讓付費使用者雙擊即開（NewPlan §16.2）。只出 Apple Silicon。AGPL-3.0 LICENSE 已落地（LICENSE 全文＋package.json／Cargo.toml 標 AGPL-3.0-only）。

規格細節（已鋪好的前置（mvp-7 2026-07-22））見 [plans/release-1-mac-signing.md](../plans/release-1-mac-signing.md)。

## Next action
- 首發不簽章，沿用 ad-hoc 簽章，發布說明附 Gatekeeper 繞過步驟；等使用者變多再考慮開工〔作者裁決 2026-10-04：會用 AI／SillyTavern 的 Mac 玩家多半熟悉簽章問題〕
- 開工時：加入 Apple Developer Program（99 美元/年），設 Developer ID 憑證＋notarytool 公證流程，憑證併入 CI secrets

## Constraints
不上 App Store；公證走全自動 notarytool，可整合 CI；只出 aarch64，不做 Intel／universal（NewPlan §16.2）。
