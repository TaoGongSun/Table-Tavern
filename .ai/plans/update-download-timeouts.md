# update-download-timeouts 做法（已結案 2026-10-11，三方驗收 PASS）

## 一、現況（查證結果）

| 呼叫點 | 行為 | 檔案大小 | 玩家看到 |
|---|---|---|---|
| `commands/update.rs` `plugin.check()` | 外掛（reqwest 0.13）每次自建 client，`UpdaterBuilder::timeout` 預設 None；送出、跟 github.com → CDN 的 302、讀 JSON 本文全無時限 | `latest.json` 數 KB | 手動檢查：「檢查更新失敗：{原文}」；自動檢查只記 log。卡住＝永遠轉圈 |
| `commands/update.rs` `update.download()` | 同一 client 設定；`check()` 建出的 `Update.timeout` 寫死 None（外掛原始碼 updater.rs:688），所以 builder 的 timeout 不會傳到下載 | 安裝包 Mac .app.tar.gz、Windows NSIS，約 5–25 MB | 「更新失敗：{原文}」；卡住＝進度條永遠停住，下載中旗標不放 |
| `commands/update.rs` `fetch_release_asset`（回退點本體＋`.sig`） | reqwest 0.12 自建 client，送出與 `bytes()` 無限等 | 同上一版安裝包 | 失敗被吞成 `rollback_ready: false`（只記 log），畫面問「沒有回退點，仍要更新嗎」；卡住＝下載完新版後永遠不結束 |

reqwest 錯誤原文（`error sending request for url (…)`、`request or response body error`）不說是逾時。

## 二、時限〔模型判斷·未裁決〕

- **閒置時限 60 秒**，三處共用：同一個 `read_timeout` 對「連線＋TLS＋所有轉址＋等回應頭」是單一期限，對本文是每收到一個 frame 就重算的閒置期限。60 秒零位元組＝連線已死；遠低於玩家耐性上限 5 分鐘（api-request-header-timeout）。慢但持續的大檔不會被殺。
- **更新檢查每個 endpoint 請求 60 秒（目前單一 endpoint）**：`UpdaterBuilder::timeout` 是 reqwest 總期限，只套 `check()`（`Update.timeout` 恆為 None），擋涓流。
- 下載不設整段期限；不開玩家設定。

## 三、包法

查證（reqwest 0.12.28 與 0.13.4 相同）：`read_timeout` 在 `PendingRequest` 是送出時建一次的 sleep，到回應頭前不重設；本文包 `ReadTimeoutBody`，每個 frame 就緒就重算。`timeout` 是連線到本文結束的總期限。

- `updater/net.rs`：
  - `NetTimeouts { check_total, idle }`，`PRODUCTION` 60／60。
  - `with_timeouts(UpdaterBuilder)`：`.timeout(check_total).configure_client(|b| b.read_timeout(idle))`；`configure_client` 存在外掛 context，`check()` 建出的 `Update` 帶著走，下載也吃到閒置期限。
  - `plugin_error_text`：`Error::Reqwest` 且 `is_timeout() || is_connect() || is_body() || is_request()`（不含 `is_decode`）→ `log::warn!` 原文後回 `UiMsg::UpdateNetworkFailed`；其餘原文。
  - `fetch_release_asset`（回退點）：擋非 https 後走 `fetch_with(url, FetchConfig)`，client 設 `redirect(Policy::limited(5))`、`https_only`（轉址目標也生效；只有測試關掉）、`read_timeout(idle)`。GitHub 資產網址一律 302 到 CDN，跟轉址才補得到（原計畫「不跟重導向」未經作者裁決）；完整性靠驗簽。傳輸錯誤以 Debug 原文進 log。
- `commands/update.rs`：改用 `with_timeouts(app.updater_builder(), PRODUCTION).build()`；`check()`／`download()` 錯誤走 `plugin_error_text`；補回退點前發一次 `update-progress {downloaded: 0, total: null}`，進度條轉不定進度，不停在 100%。
- 文案 `update_network_failed` 十語系，繁中「網路連線失敗或逾時，請檢查網路後再試一次」；玩家看到「檢查更新失敗／更新失敗：…」。回退點失敗照舊轉成「沒有回退點」詢問。

## 四、測試（`updater/net_tests.rs`，13 項）

假伺服器沿用 `transport/test_support` 的 `hang_server`、`stall_server_after`、`watchdog`（10 秒）；卡住類伺服器撐 30 秒，成功類 `hold_ms` 0。

- 正式值：60／60、回退點 `https_only` 開；`https_only` 拒 http 網址。
- 回退點：回應頭不來→逾時；半截本文後停→逾時；每 60 ms 一段、總長為閒置窗 8 倍→成功且位元組齊全；302→200 拿到本文；6 跳轉址→轉址錯誤；本機 https 假伺服器（自簽憑證、dev-dep rustls）302 指向 http：`https_only` 開→轉址錯誤，關→成功（對照組）。
- 外掛（tauri `test` feature 的 mock app 掛 updater 外掛，`dangerousInsecureTransportProtocol`、`.target("test")`、`.no_proxy()`，manifest 版本 9.9.9）：check 回應頭不來→逾時碼；JSON 每 100 ms 一字、整段 400 ms＜閒置 500 ms→逾時碼（總期限殺的）；download 半截後停→逾時碼；download 每 100 ms 一段共 3 秒→完整收到 300 位元組後是驗簽錯誤而非逾時。
- 拿掉時限／改回 `Policy::none()` 時 7 項紅；把總期限套到下載時最後一項紅（已實測）。

## 五、已知限制〔模型判斷·未裁決〕

- 每次間隔都短於 60 秒的持續涓流，安裝包與回退點下載不會逾時，不保證 5 分鐘上限。
- 休眠喚醒：macOS 喚醒後計時重新等滿；Windows 可能一醒就誤報逾時，重試即可。
- 回退點 client（reqwest 0.12、無 system-proxy、webpki 根憑證）與外掛的網路環境不一致：系統代理或 TLS 攔截環境下回退點會失敗，退回「沒有回退點」詢問。
- 逾時只斷本地連線；真的停頓超過 60 秒的玩家會看到錯誤，重試即可。自動檢查失敗照舊只記 log。
