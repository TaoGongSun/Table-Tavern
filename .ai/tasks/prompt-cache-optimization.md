# Task
Task-ID: prompt-cache-optimization
Title: 提示詞快取優化：穩定前綴重構＋命中率量測＋Claude 顯式斷點
Status: in-progress
Created: 2026-08-03T15:14:00+08:00
Updated: 2026-08-04T07:40:00+08:00

## Summary

2026-08-03 分析提示詞快取行為後開任務：system prompt 內嵌 keyword 條目與 GM 動態狀態，前綴每輪被打破、Anthropic 系命中率恆 0。同日深夜拍板改走 **claude CLI resume 續聊架構**（案 C：lane 分線＋凍結 system＋每輪只送新訊息＋回合後改寫 session 檔），範圍只做 claude；OpenRouter／API 的穩定前綴重構與顯式斷點擱置。完整架構與拆包見交接檔。

## Next action
- 現況：**包 1–7 程式面全部完成**、Opus 四輪真桌驗收全過（85–88% 命中）＝架構驗收通過；Sonnet 命中率受 claude CLI 官方 bug（#29966）壓制，app 端無事可做。細節見交接檔 Completed。
- **下一步：實機驗收**（使用者指定另開對話進行）——包 6 額度分頁八項＋包 7 前端保溫計時器，兩份清單見交接檔 Verification，可同一場真桌一起驗（保溫花費會出現在額度分頁的 ping 列）。

## 待拍板
- 補丁塊與私設後置的遵循度（實機驗收比對，明顯變差再議）。

## Constraints
- 送進模型的資訊總量與可見性規則不變：GM 專有條目永不進角色 context、私有設定規則照舊（transport.rs 頂部註解與 KICKOFF §4）。
- 一切組裝仍走 assemble_messages 家族，不得旁路。
- 快取讓位給品質：任何搬移若實測敘事品質明顯變差，寧可不搬。
- 換場摘要壓縮 transcript 必然重置快取，屬預期行為，不處理。
