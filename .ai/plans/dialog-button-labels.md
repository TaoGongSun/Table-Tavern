# 對話窗按鈕跟上介面語言

`@tauri-apps/plugin-dialog` 的 `confirm`／`message` 沒帶 `okLabel`／`cancelLabel` 時，macOS 固定顯示英文 OK／Cancel。全部補上依語意的按鈕文字，十語系齊。

## 範圍

20 處 `confirm`＋10 處 `message`；updater 已帶文字的 2 處 confirm、1 處 message 不動文字（message 仍用「關閉」），只把 `updateCancel` 換成共用鍵。範圍含 message〔作者裁決 2026-10-02〕。

## 新鍵（zh-TW 正典，放核心字典）

共用：
- `dialogCancel`「取消」——一般 confirm 的取消鈕（未儲存離開除外）；updater 的 `updateCancel` 併入後刪鍵。
- `dialogDelete`「刪除」、`dialogRemove`「移除」。
- `dialogAck`「知道了」——本次補上的 10 處通知 message 單鈕。
- `unsavedLeaveOk`「捨棄修改」／`unsavedLeaveCancel`「繼續編輯」——四處未儲存離開。

專屬：
- `useBackupOk`「改用備份」、`undoLastImportOk`「復原匯入」、`refactorRerunOk`「再跑一次」、`renameOk`「改名」。

沿用既有鍵當確認鈕：`sceneForkTitle`「從這一幕繼續」、`worldbookDedupe`「清理重複」、`convertCardToEntry`「轉成世界書條目」。

## 逐處對照

| 位置 | ok | cancel |
|---|---|---|
| App.tsx 改用備份 | useBackupOk | dialogCancel |
| App.tsx 刪桌 | dialogDelete | dialogCancel |
| App.tsx 復原匯入 | undoLastImportOk | dialogCancel |
| useCharacterController.ts 刪角色 ×2 | dialogDelete | dialogCancel |
| useSceneActions.ts 從這一幕繼續 | sceneForkTitle | dialogCancel |
| useRefactorWorkflow.ts 重跑重構 | refactorRerunOk | dialogCancel |
| VersionStoreSections.tsx 刪版本、刪備份 | dialogDelete | dialogCancel |
| useUpdateController.ts、useVersionCenter.ts | （維持 updateContinue） | dialogCancel |
| useWorldbookEditor.ts 未儲存離開 | unsavedLeaveOk | unsavedLeaveCancel |
| useWorldbookEditor.ts 刪條目 | dialogDelete | dialogCancel |
| useWorldbookEditor.ts 清理重複 | worldbookDedupe | dialogCancel |
| CardEditor.tsx 刪生成圖 | dialogDelete | dialogCancel |
| CardEditor.tsx 改名 | renameOk | dialogCancel |
| CardEditor.tsx 未儲存離開 | unsavedLeaveOk | unsavedLeaveCancel |
| CardEditor.tsx 轉世界書條目 | convertCardToEntry | dialogCancel |
| CardEditor.tsx 移除圖片、移除頭像 | dialogRemove | dialogCancel |
| SettingsWindow.tsx、WorldEditor.tsx 未儲存離開 | unsavedLeaveOk | unsavedLeaveCancel |
| 全部 10 處 message（App ×3、useImportController ×3、useRefactorWorkflow ×1、CardEditor ×3） | dialogAck | — |

## 驗收

`npm run check:i18n`、`npm run verify` 全綠；逐一核對 22 處 confirm＋11 處 message 都帶按鈕文字（含原本沒有 options 的 `showMessage(t("convertCardDone"))`）。check:i18n 的寬度掃描不涵蓋原生對話窗，長譯文要 macOS 實機看。
