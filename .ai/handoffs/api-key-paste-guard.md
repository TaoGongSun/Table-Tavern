# 金鑰貼錯防呆：貼成文件裡的指令時當場提示，401 依傳輸分流指路

Status: in-progress

## Summary
使用者按了 OpenRouter 文件 Quick Start 的 Copy，存進 app 的是 `export OPENROUTER_API_KEY=sk-or-v1-...` 這串示範指令（不是金鑰），發言時只換得一句 401「Missing Authentication header」，看不出錯在哪。新增 `src/api-key-check.ts` 純函式在輸入當下比對形狀並提示，另把 401 文案依傳輸分流。commit `a782027`。

## Progress
- 12 條形狀規則，一律**軟提示不擋存檔**：app 可接任意 OpenAI-compatible 端點，自架端點的金鑰格式無從假設，且 OpenRouter 官方文件並未明文保證 `sk-or-` 前綴不變。
- 前綴規則只在實際打到 OpenRouter 時才套（base URL 留空，或 hostname 是 `openrouter.ai` 與其子網域）。
- 設定頁與首次設定的 Onboarding 共用同一個純函式（Sol 指出 Onboarding 是同一條保存路徑，原本會漏）。
- placeholder 從 `sk-or-...` 改成「貼上在 OpenRouter 建立的完整金鑰」。
- `explainAiError(raw, transport?)` 三分流：api→換金鑰、四個已知 CLI→重新登入、拿不到來源→中性文案。
- Sol 兩輪驗收抓到三個誤判並修掉：純 Base64（`YWJjZGU=`）被當變數賦值、`notopenrouter.ai` 被 `endsWith` 誤認、未知 transport 被硬當成 CLI。
- 自驗：vitest 149（新增 8）／build／check:i18n 十語系／cargo 506 全綠。
- **實機驗收 2026-08-21 通過（兩項）**：①設定頁貼著那串示範指令時，金鑰欄正下方即時出紅字（不需先儲存）；②聊天畫面的 401 已顯示 API 版指路「金鑰可能貼錯、已失效，或這個 base URL 不接受它」，不再是誤導的「到設定重新連線」。

## Next action
本案功能面已完成。剩「換上真金鑰後紅字消失、發言成功」這一項，會在使用者實際建立 OpenRouter 金鑰時自然驗到。
