# 世界設定編輯中從側欄刪角色，未儲存內容被靜默丟掉

Status: todo〔作者裁決 2026-10-11：立案〕

## Summary
WorldEditor 開著時，從側欄隱藏區刪角色，流程走 `finishRemoval` → `setMainView(null)` 直接卸載 WorldEditor，不經 `canLeaveEditor`，沒存的 world.md 與新條目草稿會無聲遺失。換桌、換幕等入口已接守門（leave-guard-switch-table），這條沒接到。

## Next action
未排程。開工先追刪角色流程，決定守門放哪（刪除確認前先問未儲存，或刪的不是正在編輯的東西時不卸載 WorldEditor），再實作。

## Constraints
- 一併查 CardEditor 開著未存時，從側欄刪「別張」卡或刪「這張」卡是否也會靜默丟草稿。
