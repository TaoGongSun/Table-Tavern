# 重構路徑其餘靜默略過

Status: todo

## Summary
refactor-probe-failure-hidden 審查順查到、該案不處理的三處：`import/interface.rs:60` 讀原卡／PNG 解碼／JSON 解析失敗直接跳過（重構拿它當路由依據時可能誤落 none 跑角色線）；`refactor_ai/context.rs:40` 略過讀不到的既有角色（脈絡不完整無提示）；前端 `cancelAiRefactor` 的 `void invoke` 沒有錯誤處理（取消失敗無回饋）。

## Next action
未排程。
