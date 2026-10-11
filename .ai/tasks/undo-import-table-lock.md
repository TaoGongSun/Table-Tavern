# 匯入進行中按「撤銷上次匯入」可能撤錯

Status: todo

## Summary
import-refresh-world-editor 審查發現（既有問題）：撤銷只過 `undoWait`、沒過 `runTableOp`；陣容欄撤銷項在 `tableOp.busy` 時也不停用。匯入排在回合後面時按撤銷，兩者等回合結束後的後端順序不固定：可能撤掉剛匯入的卡、確認框卻顯示上一筆的名稱；匯入流程接著對已刪的卡做 focus、開場白、介面提示。

## Next action
未排程。修法候選：`undoLastImport` 開頭查 `tableOp.isHeld()`，或撤銷項依 `tableOpBusy` 停用。
