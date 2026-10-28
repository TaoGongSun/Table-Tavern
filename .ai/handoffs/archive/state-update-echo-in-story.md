> 結案 2026-10-28：Sol、opus-review 驗收 PASS，squash 進 main。

# state-update-echo-in-story — GM 把歷史裡的「（系統）狀態更新」事件照抄進正文

## 根因
長文字欄（inject=Snapshot，MVU 桌無規則的文字欄預設如此）的變動落成系統事件留在歷史，送模型時標頭是「（系統）狀態更新」。卡片或世界書要求每回合附狀態回報、導演指示又要求照設定格式附在旁白後時，模型把這段當成狀態欄格式照抄。重現集中於 hroot2；一般 MVU 桌 Sonnet 21 回合 0 次。

## 做法（三方共識，規格見 [plans](../../plans/state-update-echo-in-story.md)）
- C：`transport::arrivals::prompt_text` 對狀態變動事件改用送模型專用標頭 `data::prompt_state_heading`（「App 內部帳本……不是回覆格式」），UI 與匯出字典不動；事件照留歷史（state-values-mvu 包 5 快取設計不變）。
- E：`transport::strip_state_echo` 在 `gm_narrate`（`gm_reply_parts`）剝 GM 正文尾端照抄段，條件全成立才剝；程式碼區塊判斷用 pulldown-cmark；raw 與機制輸入照用原文。

## 實測（2026-10-28）
hroot2 重播：Sonnet 原樣 12/20 → 0/20；agy Gemini 3.8 Flash Medium 10/10 → 0/10；現值更新照常。

## 已知限制
見 plans 已知限制段（英文標頭未實測、舊 session 標頭重開才換、E 只剝 GM 正文等）。
