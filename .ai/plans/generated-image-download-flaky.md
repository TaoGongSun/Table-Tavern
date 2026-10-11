# 生圖下載測試偶發逾時：根因與修法

## 根因
`download()` 的 5 秒 `tokio::time::timeout` 把 `reqwest::Client::new()` 也包了進去（原 `src-tauri/src/generated_image.rs:82`、`:89`）。macOS 上，同一行程第一次 `Client::new()` 會查系統代理，期間掃描執行檔所在的目錄，花掉好幾秒，這段時間全算進下載期限。

呼叫鏈：reqwest 0.12 `ClientBuilder::build` → `Matcher::system()` → hyper-util `mac::with_system` → `SCDynamicStoreCreate` → `_SC_getApplicationBundleID`（dispatch_once）→ `CFBundleGetMainBundle`。測試執行檔不在 .app 裡，所以會對它所在的目錄做 `readdir`。系統代理查詢之所以開著，是 tauri-plugin-updater 開了 reqwest 0.13 的 `system-proxy`，hyper-util 的 `client-proxy-system` 經 feature 統一，也套到了 reqwest 0.12。

證據：
- `sample` 抓卡住的測試行程：653 個樣本中有 652 個落在 `Client::new` 這條鏈上，其中 `__getdirentries64` 合計 544 個。
- `target/debug/deps` 約有 61 萬個檔案、104G。同一個執行檔放在 deps 底下，`Client::new()` 要 1.36–4.39 秒；複製到空目錄只要 3–5 毫秒。
- 因為卡在 dispatch_once，同時起跑的下載測試會一起等。會紅的剛好是期限 5 秒、預期會成功或快速失敗的那幾條。

已排除：proxy 環境變數、系統代理設定、測試裡的 `set_var`、DNS、假伺服器 accept 的時機。

正式版：.app 的執行檔目錄很小，Windows 則是讀登錄檔，都不會掃大目錄；但這段同步初始化每個行程仍會跑一次，通常較快，實際耗時未實測。

## 修法（已實作）
1. `download()` 先建 client 再計時，`download_body` 改成收 `&reqwest::Client`。期限只管送出請求到讀完 body，時限不放寬。
2. `transport/test_support.rs` 新增 `warm_http_client()`：用 `std::sync::Once` 包住一次 `reqwest::Client::new()`。
   - 呼叫位置：`watchdog` 計時前，以及 `transport/client/tests.rs` 兩條量 `started.elapsed() < 5s` 的測試，在 `Instant::now()` 前。
   - `watchdog` 的使用者都因此受益：updater/net_tests、openrouter_oauth、transport/responses、transport/client/timeout_tests。
3. 改正錯誤的註解。原本寫「載入系統憑證」，但專案用的是 rustls-tls＋webpki-roots，不讀系統憑證，真正的原因是查系統代理。改了四處：`openrouter_oauth.rs`、`transport/client.rs` 兩處、`transport/client/tests.rs` 的 `stall_keepalive_comments_do_not_extend_deadline` 說明。
4. `download_times_out` 的斷言收緊成 `AI_IMAGE_DOWNLOAD_FAILED: timed out`。

這次的修法只讓初始化耗時不再算進期限，同步初始化仍會短暫卡住 runtime，這點沒有處理〔模型判斷·未裁決〕。

## 範圍外
- 改成共用一個 Client，屬另案，這次不做〔模型判斷·未裁決〕；Client 建立維持全 repo 一致的 `Client::new()`。
- `target/debug/deps` 過大，建議作者自行 `cargo clean`。這只是減輕症狀，不能取代上面的修法。

## 驗證（2026-10-12）
確定性重現的做法：每輪新建一個塞了 60 萬個空檔的目錄，把測試執行檔複製進去跑首輪（同一目錄重跑時快取已熱，不會重現），執行 `generated_image::`、`transport::client::tests::generate_image*`、`key_tier_for_times_out_when_body_stalls`，跑完把目錄刪掉。

- 修前：3/3 輪紅，每輪紅 5 條（4 條 `download_*` 是 `timed out after 5s`，加上 `generate_image_key_lookup_times_out_when_body_stalls`）。
- 修後：3/3 輪綠（26 passed）。
- 加臨時計時（未 commit）：`Client::new` 等了 1.62 秒，計時從那之後才開始。
- `npm run verify`：12 步全綠。
