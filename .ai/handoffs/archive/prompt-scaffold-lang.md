# 送 AI 的提示詞骨架改成「中文介面送繁中、其餘送英文」

Status: done（2026-10-03 進 main，Sol 審查與驗收同意）。規格見 [plans/prompt-scaffold-lang.md](../../plans/prompt-scaffold-lang.md)。

## 立案說明
〔作者裁決 2026-10-03〕送給 AI 的固定提示詞骨架只有 zh* 介面送繁中，其餘語系（含未知）一律送英文，避免非中文介面被整份繁中提示詞帶出中文腔。輸出語言照舊由 `transport/messages.rs` `language_rule` 決定，不做每語系各一套提示詞。顯示、匯出、匯入段標、`player_fallback_name` 等照介面語系的不動。

## 現況
- 分支 `prompt-scaffold-lang`：A＋B1 實作完成，zh 骨架與改動前基準逐字相同；verify 10 步全綠（vitest 668、cargo 803、harness 28）。

## 下一步
無。
