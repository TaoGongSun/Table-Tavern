# generated-image-download-flaky — 生圖下載測試偶發逾時（已結案）

規格（根因、修法、驗證）：[plans/generated-image-download-flaky.md](../../plans/generated-image-download-flaky.md)。

## 結案摘要
- 根因：macOS 上第一次 `reqwest::Client::new()` 查系統代理時會掃執行檔所在目錄；`target/debug/deps` 檔案極多，這幾秒被算進 `download()` 的 5 秒期限。
- 修法：`download()` 把 client 移到期限外再建；測試另加共用的 `warm_http_client()`，`watchdog` 與兩條量時間的測試在開始計時前呼叫；順手更正「載入系統憑證」的錯誤歸因。
- 驗證：確定性重現修前 3/3 紅、修後 3/3 綠；verify 12 步綠；三方驗收 PASS。
