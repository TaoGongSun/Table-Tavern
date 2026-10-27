# MVU 墊片事件測試在負載下偶發失敗

分支 `mvu-shim-flaky-test`：「async 監聽器依序等完」測試改等最後一個 ENDED 監聽器的 Promise（金=11 才 resolve），不再靠 `setTimeout(60)`。只改測試，產品碼未動。

## 下一步
主線檢查後壓縮合併進 main。
