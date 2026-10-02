# desktop-update-detect — 桌面版 App 內更新與回退

分支：`desktop-update-detect`（已 push）

## 現況
- 設計已拍板，規格在 [plans/desktop-update-detect.md](../plans/desktop-update-detect.md)。Mac 只出 Apple Silicon。
- 包 1（發版管線）完成：`test-v0.2.0` 演練全綠（2026-10-01）。
- 包 2（格式版本與遷移）完成：Grok 實作，Opus、Sol 驗收三方共識（2026-10-02，Sol 驗收串 `01a0f87f-1392-7f01-a8d1-adabfe205b5f`）。`npm run verify` 全綠（vitest 194、cargo test 609）。沒有跑 GUI、沒有打包；中斷與 Windows／Mac 情境排在計畫「驗收」的實機項。
- 待作者裁決：唯讀桌能不能刪（目前可以，見計畫「實作時補定的細節」）。
- 包 3–5 還沒有程式。包 2–5 首次公開時必須同版發出。

## 下一步
- 包 3（偵測與一鍵更新）：先寫做法，送 Sol、Grok 審到共識，再派 Grok 實作。
