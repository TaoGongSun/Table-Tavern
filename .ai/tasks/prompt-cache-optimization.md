# Task
Task-ID: prompt-cache-optimization
Title: 提示詞快取優化：穩定前綴重構＋命中率量測＋Claude 顯式斷點
Status: in-progress
Created: 2026-08-03T15:14:00+08:00
Updated: 2026-08-03T15:55:00+08:00

## Summary

2026-08-03 與使用者在 Cowork 對話分析提示詞快取行為後開任務。現況：`stream_chat` 只送 model／messages／stream，無任何 `cache_control`；經 OpenRouter 時，GPT／DeepSeek／Gemini 2.5／Grok 等「隱式快取」模型靠前綴相同自動命中，Anthropic 系模型需顯式斷點、目前命中率恆為 0。另有兩個系統性打破前綴的設計：(1) 世界書 keyword 條目由最近 4 則事件掃描決定進出、且拼在 system prompt（整個 context 第一段），條目一翻動後面全滅；(2) GM 的 system prompt 內嵌「目前狀態」（時間／地點／在場人物），每輪旁白後更新，GM 幾乎每輪全額重算——而 GM 是呼叫最頻繁的檔位。角色路徑本身乾淨（system 穩定＋transcript append-only）。

拍板三塊，依 A→C→B 順序：

- **A 穩定前綴重構**：keyword 觸發的世界書條目與 GM「目前狀態」移出 system prompt，改組裝成 transcript 尾端（最新事件附近）的一則 user 訊息；constant 條目、world.md、角色卡留在 system。目標：連續兩輪呼叫，除尾端動態塊與最新事件外，messages 前綴逐字相同。
- **C 命中率量測**：請求加 `"usage": {"include": true}`，解析 SSE 尾塊的 `usage.prompt_tokens_details`（cached_tokens 等），先記 log／除錯顯示驗證 A 的效果；正式 UI 待拍板。
- **B Claude 顯式斷點**：`ChatMessage` content 支援 multipart 陣列，模型 id 屬 anthropic 系時在穩定前綴尾標 `cache_control: {"type": "ephemeral"}`；其他模型 request 形狀零變化。

## Next action
- A＋C 已完成（2026-08-03，cargo test 167 全綠；細節與行號見 handoffs/prompt-cache-optimization.md）。接著：實機驗收——終端機啟動 app、同桌連續兩輪看 stderr 的 `[prompt-cache]` 命中率，並比對搬尾端後的敘事品質（變差則回退）。B 等拍板優先度。
- 注意：C 查證後修正一項分析假設——OpenRouter 不回報快取寫入 token 數（官方文件明言不支援），只有讀取命中 cached_tokens；usage accounting 只對 OpenRouter 端點帶，其他端點請求形狀不變。

## 待拍板
- 條目與狀態搬到尾端對模型遵循度的影響（A 完成後實測比對；若明顯變差，該項改回 system 並記錄取捨）。
- 命中率顯示的正式 UI 位置（對話頁角落 vs 設定／除錯區）與是否對一般使用者可見。
- B 的優先度：使用者目前檔位若不用 Claude 系模型可延後。

## Constraints
- 送進模型的資訊總量與可見性規則不變：GM 專有條目永不進角色 context、私有設定規則照舊（transport.rs 頂部註解與 KICKOFF §4）。
- 一切組裝仍走 assemble_messages 家族，不得旁路。
- 快取讓位給品質：任何搬移若實測敘事品質明顯變差，寧可不搬。
- 換場摘要壓縮 transcript 必然重置快取，屬預期行為，不處理。
