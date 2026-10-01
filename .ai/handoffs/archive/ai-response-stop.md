# ai-response-stop — 對話中止 AI 回應

規格（拍板、做法、驗收）：[plans/ai-response-stop.md](../../plans/ai-response-stop.md)。

## 現況
- 實作完成在分支 `ai-response-stop`，`npm run verify` 全綠（cargo test 571、vitest 162）；Opus、Sol、Grok 三方驗收共識。
- 生成中送出鍵原位變「停止」；前端唯一入口 `useChatController.stopResponse()`（日後更新閘門直接呼叫）。後端 `chat_abort(world_id, turn_id)`，inflight 鑰匙 `(Kind, world_id)` 與重構隔離。
- lane 中止等子程序退出後抹私設；抹不掉棄 session，刪檔也失敗就清線並回錯。中止不更新 `expected_reply`，下輪由正典重建。
- 換幕摘要（`beginNarration`）沒有 turn_id，不給停止鍵。〔模型判斷·未裁決〕

## 下一步
- 實機驗收（[驗證佇列](../../reference/verification-queue.md)梯 2 第 11）；過了照規約 squash 進 main 結案。
