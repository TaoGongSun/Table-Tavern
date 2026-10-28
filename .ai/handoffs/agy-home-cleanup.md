# agy-home-cleanup — app 專用 agy 家目錄的對話清理與上游追蹤

## Summary
agy-conversation-isolation（作者裁決 A1，2026-10-28）讓 app 的 agy 對話改存 `<config_root>/agy-home`。這些對話只會累積，`lanes/cleanup.rs` 對 agy 不做任何清理；隔離後是 app 自己的資料，清理不再有隱私顧慮。另：agy 沒有資料目錄變數，我們靠換 HOME＋keychain symlink，agy 改版可能失效。

## Next action
未排程，等 agy-conversation-isolation 結案。① 比照其他 lane 的清理規則處理 agy-home 的對話（刪桌／lane 重開時）；② 追蹤向 Google 提「資料目錄變數」需求（由作者決定是否提出），有了就改成一行設定取代 symlink。

## Constraints
- 只清 agy-home，不碰使用者自己的 ~/.gemini。
