> 結案 2026-10-11：squash 進 main，Opus、Opus、Sol 三方驗收通過。

# 編輯中從側欄刪角色，未儲存內容被靜默丟掉

Status: done

## Summary
刪的不是編輯中的東西就不關編輯器，刪這張才關；隱藏／還原前先問未存；WorldEditor 收到刪除通知只重讀並修剪草稿；前端存條目時後端剪掉卡檔確定已不在的角色 id，刪卡＋清理持同桌提交鎖。定案結論見 [plans/world-editor-delete-character-guard.md](../../plans/world-editor-delete-character-guard.md)。

## 驗收
verify 12 步綠（cargo test 1607、vitest 1178、web vitest 774）；測試通道零 AI 五步實測過。
