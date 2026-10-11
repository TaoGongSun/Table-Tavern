# web-autosave-test-flaky — web 自動存檔測試偶發失敗（已結案）

規格（根因、修法、驗證）：[plans/web-autosave-test-flaky.md](../../plans/web-autosave-test-flaky.md)。

## 結案摘要
- 根因：測試寫法問題。「first slot must land before the first dispatch」放行首格寫入後只固定等 20ms 就斷言已送模；負載下寫首格加送模流程超過 20ms 就紅。產品沒有競態：首格寫成前一發都不送。
- 修法：只改測試。放行後改用 waitFor 等到送模。
- 驗證：模擬負載確定性重現修前紅、修後 0／30／200ms 都綠；反向對照「不等首格就送」與「寫成後不送」都抓得到；verify 12 步綠；小修改免三方審查。
