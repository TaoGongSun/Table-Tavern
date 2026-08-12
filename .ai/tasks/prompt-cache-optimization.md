# Task
Task-ID: prompt-cache-optimization
Title: 提示詞快取優化：resume 續聊架構（claude lane）
Status: in-progress
Created: 2026-08-13T00:30:01.008157+08:00
Updated: 2026-08-13T00:30:01.008157+08:00

## Summary
2026-08-03 分析提示詞快取行為後開任務：system prompt 內嵌 keyword 條目與 GM 動態狀態，前綴每輪被打破、Anthropic 系命中率恆 0。同日深夜拍板改走 **claude CLI resume 續聊架構**（案 C：lane 分線＋凍結 system＋每輪只送新訊息＋回合後改寫 session 檔），範圍只做 claude；OpenRouter／API 的穩定前綴重構與顯式斷點擱置。完整架構與拆包見交接檔。

規格細節（待拍板）見 [plans/prompt-cache-optimization.md](../plans/prompt-cache-optimization.md)。

## Next action
- 本任務主體完成——包 1–7 全數實作並通過實機驗收（架構 85–88% 命中、額度分頁九項過、保溫 ping 94.6%）；2026-08-06 額度分頁改成「已省 X% 費用／約省下 $Y」口徑並實機看過；剩 grok／agy 顯示驗收延後與 OpenRouter 計量未接，見交接檔 Remaining

## Constraints
- 送進模型的資訊總量與可見性規則不變：GM 專有條目永不進角色 context、私有設定規則照舊（transport.rs 頂部註解與 KICKOFF §4）。
- 一切組裝仍走 assemble_messages 家族，不得旁路。
- 快取讓位給品質：任何搬移若實測敘事品質明顯變差，寧可不搬。
- 換場摘要壓縮 transcript 必然重置快取，屬預期行為，不處理。
