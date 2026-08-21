# 串流失敗看得見：零內容／異常收尾轉成玩家看得懂的錯誤，不落故事

Status: in-progress

## Summary
OpenRouter 串流「正常走完但零內容」目前無聲成功：空回應被寫進 transcript 當正常 GM 回合、進入後續呼叫歷史污染上下文、還照樣重擲一輪骰。改成四段防守＋三個錯誤碼，讓玩家看到人話錯誤且故事不被寫入任何東西。規格與拍板結論見 [.ai/plans/stream-failure-visible.md](../plans/stream-failure-visible.md)。

## Progress
2026-08-21 立案；與 Sol 三輪討論收斂完成，規格已定。

## Next action
包 1：transport.rs stream_chat 收工判定（SSE error 原樣拋、finish_reason 分流、trim 後空判定）

## Constraints
不動 `DataResult<String>` 回傳型別、不改 20 個 `stream_via_transport` 呼叫點；不做自動重試與拒絕偵測。
