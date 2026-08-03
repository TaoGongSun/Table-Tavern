# Task
Task-ID: prompt-cache-optimization
Title: 提示詞快取優化：穩定前綴重構＋命中率量測＋Claude 顯式斷點
Status: in-progress
Created: 2026-08-03T15:14:00+08:00
Updated: 2026-08-03T23:59:00+08:00

## Summary

2026-08-03 與使用者在 Cowork 對話分析提示詞快取行為後開任務。現況：`stream_chat` 只送 model／messages／stream，無任何 `cache_control`；經 OpenRouter 時，GPT／DeepSeek／Gemini 2.5／Grok 等「隱式快取」模型靠前綴相同自動命中，Anthropic 系模型需顯式斷點、目前命中率恆為 0。另有兩個系統性打破前綴的設計：(1) 世界書 keyword 條目由最近 4 則事件掃描決定進出、且拼在 system prompt（整個 context 第一段），條目一翻動後面全滅；(2) GM 的 system prompt 內嵌「目前狀態」（時間／地點／在場人物），每輪旁白後更新，GM 幾乎每輪全額重算——而 GM 是呼叫最頻繁的檔位。角色路徑本身乾淨（system 穩定＋transcript append-only）。

拍板三塊，依 A→C→B 順序：

- **A 穩定前綴重構**：keyword 觸發的世界書條目與 GM「目前狀態」移出 system prompt，改組裝成 transcript 尾端（最新事件附近）的一則 user 訊息；constant 條目、world.md、角色卡留在 system。目標：連續兩輪呼叫，除尾端動態塊與最新事件外，messages 前綴逐字相同。
- **C 命中率量測**：請求加 `"usage": {"include": true}`，解析 SSE 尾塊的 `usage.prompt_tokens_details`（cached_tokens 等），先記 log／除錯顯示驗證 A 的效果；正式 UI 待拍板。
- **B Claude 顯式斷點**：`ChatMessage` content 支援 multipart 陣列，模型 id 屬 anthropic 系時在穩定前綴尾標 `cache_control: {"type": "ephemeral"}`；其他模型 request 形狀零變化。

## Next action
- 2026-08-03 晚：七個對照實驗鎖死 claude CLI 快取規則（塊級匹配、5 分鐘壽命、續聊＝99.7% 命中），拍板翻案 §8.1 改 `--resume` 續聊架構＋案 C（角色共用一條 session、私設注入後從本機 session 檔抹寫）＋GM 合併呼叫＋log 升級（遠期通設定頁額度/命中率分頁）。範圍收束只做 claude，OpenRouter／API 驗收擱置。完整規格與拆包順序見交接檔。實作進行中：**包 1（session 檔操作）＋包 2（resume 流程地基）完成**（2026-08-03，cargo test 201 綠；案 C 改寫-resume 機制已實機驗證），claude 模式三個呼叫已走續聊線。下一步包 3 凍結快照補丁（先拍板「角色檔位混用打散快取」怎麼處理，見交接檔已知限制）→ 包 4 log → 包 5 GM 合併 → ping 最後拍。

## 待拍板
- 補丁塊與私設後置的遵循度（實機驗收比對，明顯變差再議）。
- 保溫 ping 的觸發細節（實作輪拍）。

## Constraints
- 送進模型的資訊總量與可見性規則不變：GM 專有條目永不進角色 context、私有設定規則照舊（transport.rs 頂部註解與 KICKOFF §4）。
- 一切組裝仍走 assemble_messages 家族，不得旁路。
- 快取讓位給品質：任何搬移若實測敘事品質明顯變差，寧可不搬。
- 換場摘要壓縮 transcript 必然重置快取，屬預期行為，不處理。
