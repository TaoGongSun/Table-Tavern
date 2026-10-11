# 撤銷上次匯入時編輯器的兩個問題

Status: todo

## Summary
world-editor-delete-character-guard 範圍外發現：
- 撤銷上次匯入用 `setWorldEditorRefreshKey` 重掛載 WorldEditor，沒存的 world.md 與新條目草稿靜默消失；開頭也沒經 `canLeaveEditor`。
- CardEditor 正開著被撤銷掉的那張卡時，編輯器不關，停在一張已刪的卡上。
另有一個小瑕疵：刪檔失敗的錯誤跨桌晚到可能顯示在新桌錯誤列〔模型判斷·未裁決〕。

## Next action
未排程。可比照 world-editor-delete-character-guard（castRemoval、進桌世代、只關被刪那張）處理。
