> 結案 2026-10-04：包 A（後端）、包 B（前端＋十語系）進 main，Sol 驗收通過；剩一項待驗排在[實測佇列](../../reference/verification-queue.md)梯 3。

# 穩定免費自動避開不能用的模型

Status: done

## Summary
首發前必做〔作者裁決 2026-10-04〕。2026-10-04 代測時穩定前兩名同屬 Google AI Studio 共用池、整段 429，新玩家一句都送不出去。規格見 [plans/stable-free-failover.md](../../plans/stable-free-failover.md)；[free-player-onboarding](../../plans/free-player-onboarding.md) §4／§6／§7／§8／§10／§12–§18 已照新規格改寫。

- 程式：`src-tauri/src/smart_free/`（failover／call／probe 新檔；select 名單 4 支分散上游、輸出只收純文字；store 目前模型檔與上游快取）、`transport/api_failure.rs`（結構化錯誤）、`transport/dispatch.rs`（穩定免費分支、事件 payload、turnId）、前端聊天室換模提示行與設定頁 4 支名單、十語系文案；測試通道 `openrouter-origin` 覆寫與 `scripts/harness-fake-openrouter.mjs`。
- 驗收：verify 10/10；測試通道假端點六項（啟動試打避開 429、連續兩次 503 換模並同句重送、提示在玩家句與回覆之間且不進逐字稿、帳本每發一行、設定頁 4 支／目前使用／降級說明、401／402 不換模且試打停損）全過；真模型試打避開 429 的 gemma、nemotron 正常回覆，自然遇到一次上游過載只回報不換模。

## 待驗（排在實測佇列）
- 真上游連續兩次失敗→換模→同句重送：只在假端點驗過，真模型自然遇到時再看。
