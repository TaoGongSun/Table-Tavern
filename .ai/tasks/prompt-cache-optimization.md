# Task
Task-ID: prompt-cache-optimization
Title: 提示詞快取優化：穩定前綴重構＋命中率量測＋Claude 顯式斷點
Status: in-progress
Created: 2026-08-03T15:14:00+08:00
Updated: 2026-08-04T02:10:00+08:00

## Summary

2026-08-03 與使用者在 Cowork 對話分析提示詞快取行為後開任務。現況：`stream_chat` 只送 model／messages／stream，無任何 `cache_control`；經 OpenRouter 時，GPT／DeepSeek／Gemini 2.5／Grok 等「隱式快取」模型靠前綴相同自動命中，Anthropic 系模型需顯式斷點、目前命中率恆為 0。另有兩個系統性打破前綴的設計：(1) 世界書 keyword 條目由最近 4 則事件掃描決定進出、且拼在 system prompt（整個 context 第一段），條目一翻動後面全滅；(2) GM 的 system prompt 內嵌「目前狀態」（時間／地點／在場人物），每輪旁白後更新，GM 幾乎每輪全額重算——而 GM 是呼叫最頻繁的檔位。角色路徑本身乾淨（system 穩定＋transcript append-only）。

拍板三塊，依 A→C→B 順序：

- **A 穩定前綴重構**：keyword 觸發的世界書條目與 GM「目前狀態」移出 system prompt，改組裝成 transcript 尾端（最新事件附近）的一則 user 訊息；constant 條目、world.md、角色卡留在 system。目標：連續兩輪呼叫，除尾端動態塊與最新事件外，messages 前綴逐字相同。
- **C 命中率量測**：請求加 `"usage": {"include": true}`，解析 SSE 尾塊的 `usage.prompt_tokens_details`（cached_tokens 等），先記 log／除錯顯示驗證 A 的效果；正式 UI 待拍板。
- **B Claude 顯式斷點**：`ChatMessage` content 支援 multipart 陣列，模型 id 屬 anthropic 系時在穩定前綴尾標 `cache_control: {"type": "ephemeral"}`；其他模型 request 形狀零變化。

## Next action
- 拍板 `--resume` 續聊架構＋案 C（角色共用一條 session、私設注入後從本機 session 檔抹寫）＋GM 合併呼叫＋結構化 log；範圍只做 claude，OpenRouter／API 驗收擱置。完整規格與拆包順序見交接檔。
- **包 1–5 全部完成且架構已驗收**（cargo test 213 綠）：claude 模式全部呼叫走續聊線，lane 按「線種:實際模型」分池，改卡／改世界書當輪不重開線（走補丁／追平），一次呼叫一行 JSONL 含八個診斷標籤與花費，GM 每輪一次呼叫同時產旁白與點名。2026-08-04 Opus 四輪真桌驗收 2–4 輪 diag=ok、85.3／87.2／88.3%，讀到量與理論可中量每輪只差 2 token。
- Sonnet 命中率被 claude CLI 官方 bug 壓制（2.1.220 仍在，只咬 Sonnet 系；官方 #29966），app 端無事可做，不發 issue。
- 換幕提醒改標準：字數門檻 8000→30000 並換掉文案理由（已做，10 語系綠）；第二標準「保溫 ping 兩次無回應＝玩家離開」隨包 7 做。
- **下一步：包 6 額度分頁 UI**（設定頁讀 JSONL 畫花費＋命中率＋燈號＋原因句），之後包 7 保溫 ping。

## 待拍板
- 補丁塊與私設後置的遵循度（實機驗收比對，明顯變差再議）。
- 保溫 ping 的觸發細節（實作輪拍）。

## Constraints
- 送進模型的資訊總量與可見性規則不變：GM 專有條目永不進角色 context、私有設定規則照舊（transport.rs 頂部註解與 KICKOFF §4）。
- 一切組裝仍走 assemble_messages 家族，不得旁路。
- 快取讓位給品質：任何搬移若實測敘事品質明顯變差，寧可不搬。
- 換場摘要壓縮 transcript 必然重置快取，屬預期行為，不處理。
