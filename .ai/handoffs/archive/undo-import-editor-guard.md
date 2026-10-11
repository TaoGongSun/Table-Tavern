> 結案 2026-10-11：squash 進 main，Opus、Opus、Sol 三方驗收通過。

# undo-import-editor-guard — 撤銷上次匯入時編輯器的兩個問題

Status: done

方案見 [plans/undo-import-editor-guard.md](../../plans/undo-import-editor-guard.md)。

## 問題
- 撤銷上次匯入用 `setWorldEditorRefreshKey` 重掛載 WorldEditor，未存的 world.md 與條目草稿靜默消失。
- CardEditor 正開著被撤銷掉的卡時不關。
- 順帶：角色 controller 的錯誤與回捲跨桌晚到會落在新桌。

## 狀態
已進 main：後端回報撤銷刪掉的角色 id 與條目改動欄組、存既有條目找不到就回 `worldbook_entry_gone`；世界設定不重新掛載，撤銷通知排待定基、重讀成功時定基草稿，待定基期間既有草稿不寫回（手動儲存、轉成角色卡、帳本開關一律先等定基）；被撤銷的卡正在編輯就關；角色 controller 進桌世代。verify 12 步綠；測試通道零 AI 六步實測過（第 6 步用的重構卡套進空桌，沒有改寫既有條目、也沒指定玩家卡，這兩項只在單元測試驗）。
