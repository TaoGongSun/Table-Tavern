# Task
Task-ID: mvu-replace-numeric
Title: MVU 卡的數字欄位 replace 被機制層拒收
Status: todo
Created: 2026-10-03
Updated: 2026-10-03

## Summary
card-mvu-shim 實機驗收 2026-10-03 發現。

現象：app 機制層解析 GM 回覆的 `<UpdateVariable>` 時，對數字欄位的 replace 一律拒收，mechanism-log 訊息大意為「現值 100000，請用 delta」，該筆數值不動。

影響：MVU 卡教 GM 的寫法常用 replace，酒館／MVU 上游會照套；app 拒收會讓數值停在原地，與「沒重構的卡照酒館行為照做」不符。

## Next action
- 查機制層為何對數字欄強制 delta（來源與當初理由）。
- 查未重構的 MVU 卡是否該照上游接受 replace，以及與重構卡／既有狀態更新協定的關係。
- 不下結論；開工前先重現（測試通道、MVU 卡 TestCards/bcd368…png 與 TestCards/DongeonMaster.png）。
- 與 gm-format-directive-missing-target（已進 main）合併相依：本案進 main 後，要核對 numeric replace 的前後端接收行為與 zh/en 協定是否一致；不得為了避開衝突而重新停用原卡的格式條目。
