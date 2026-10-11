# worldeditor-reset-test-flaky — WorldEditor 重構重設測試偶發失敗（已結案）

規格（根因、修法、驗證）：[plans/worldeditor-reset-test-flaky.md](../../plans/worldeditor-reset-test-flaky.md)。

## 結案摘要
- 根因：測試寫法問題。假後端在 `setTimeout(0)` 後才讀畫面上的表單，這個 1ms 計時器要和 React act 用 `setImmediate` 處理的 render 比先後；負載下計時器先觸發，表單還在畫面上，測試就紅。產品沒有競態：草稿在送出清回前就已同步關閉（`draftRef`）。
- 修法：只改測試。假後端改在清回回應前等表單關掉，斷言改成 `closedWhileResetPending`。
- 驗證：模擬負載的確定性重現修前 3/3 紅、修後 0／5／50ms 都綠，反向對照抓得到「清回後才關」；verify 12 步綠；小修改免三方審查。
