# desktop-update-detect — 桌面版 App 內更新與回退

分支：`desktop-update-detect`

## 現況
- 設計已全部拍板（2026-09-30），並經 Claude／Grok／Sol 三方審核達成共識。規格在 [plans/desktop-update-detect.md](../plans/desktop-update-detect.md)：方案 B 一鍵更新、App 內回退、每桌格式版本＋開到才轉、舊版遇新格式唯讀。
- Mac 只出 Apple Silicon。
- 包 1（發版管線）程式已完成：`scripts/check-version.mjs`、`.github/workflows/release.yml`（取代 test-build.yml）、`scripts/release/` 的 finalize，`plugins.updater` 已填正式公鑰；簽章 secret 已設；`test-v0.2.0` 演練全綠（兩平台打包、驗簽、latest.json，2026-10-01）。
- 包 2–5 還沒有程式。前置 [ai-response-stop](archive/ai-response-stop.md) 已完成。

## 下一步
- 包 2（格式版本與遷移）做法審核中。
- 包 2–5 首次公開時必須同版發出；計畫「實機驗證」各項要等有兩個真 release 才能測。
