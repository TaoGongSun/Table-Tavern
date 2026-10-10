# 桌面版世界書觸發補齊到 SillyTavern 行為

Status: in progress〔作者裁決 2026-10-07：立案，網頁版公開前門檻 D17〕。分支 `worldbook-st-trigger-parity`。方案見 [plans/worldbook-st-trigger-parity.md](../plans/worldbook-st-trigger-parity.md)（定稿：第 1–5 輪三方審查已納入；P1–P9、變數不追蹤私密、範例上／下併入前／後為作者裁決）。分包見方案第六節。

## 包 1 現況（掃描核心＋對拍 fixture）
- 施工完成；驗收第 1 輪必改與建議、第 2 輪兩條必改（前看下限 0 改寫成 `(?:|(?!)前看)`、V8 長度 < 8 直接二分插入）與重複群組捕捉的已知差異、第 3 輪 `{min,max}` 上下限顛倒判為轉譯失敗都已修完；第 3 輪 Opus 審查、Grok 通過，包 1 結束。
- Rust `src-tauri/src/world_info/`：`entry.rs`（fromWorldFile／fromCharacterBook、`@@` 裝飾）、`scan.rs`（checkWorldInfo＋觸發來源＋撞回溯上限的鍵快取）、`timed.rs`、`settings.rs`、`regex_key.rs`（JS 正則逐字元轉譯：量詞合法性、條件式反向參照、前看量詞改寫）、`sort.rs`（V8 TimSort 移植）、`js_semantics.rs`。尚未接任何組裝點，`mod.rs` 在非測試建置放寬 dead_code。
- 依賴：新增 `fancy-regex` 0.19.2；`getrandom` 改正式依賴。
- 對拍：`src/shared/contracts/world-info/`——scan 58、regex 110、sort 13、entry 12 案，說明在 `world-info.md`。產生：在 `web/` 下 `node scripts/gen-world-info-fixtures.mjs`（輸入 `web/scripts/world-info-fixture-cases.ts`）。測試 `web/src/features/sillytavern/world-info-parity.test.ts`、`src-tauri/src/world_info/parity_tests.rs`；Rust 限定：`source_tests.rs`（觸發來源）、`regex_key.rs`（回溯上限兩案）、`scan.rs`（上限快取）。
- 已知差異（regex-cases 的 `knownDifference`）：無 `u` 的 `/^..$/` 對 emoji、落單代理 `/\uD83D/`、`/[\w]/i` 對 ſ、`/[a-z]/i` 對 K、`/\p{Han}/u`、重複群組不清上一輪捕捉（`/^(a\1)+$/`、`/^(?:(a)|b)+\1$/`）。已寫進方案七。
- 驗收時沒結掉的建議：無（第 1 輪 7–11 已全納入）。

## 包 2 起要注意
- 包 2：V2 卡缺 `insertion_order` 時網頁版是 undefined（NaN，跟誰比都算相等）；落檔成物件形後若不寫 `order` 鍵，讀回來會變成 `fromWorldFile` 的預設 100，排序就與網頁版不同。轉換時要決定怎麼保住「undefined」（例如保留缺鍵並讓桌面版讀取端認得來源形狀，或寫入可還原 NaN 的值），並在 `entry-cases`／`sort-cases` 補落檔後讀回的案例。
- 包 2：`entry.rs` 的 `from_character_book` 已照網頁版，可直接當轉換規格；轉換後再經 `from_world_file` 讀回要得到同一個 `WiEntry`（含 `selective` 缺欄＝false、`enabled` 缺欄＝停用）。
- 包 5a：`ScanHooks`（代換、計數、亂數）由呼叫端提供；`GlobalScan` 每欄分 `full`／`public`（P7 的 `public_md`＋`private_md`）；限定條目要設 `WiEntry.limited = true`；`WiResult.private_ids`／`private_content` 是機密分流的輸入；`outlets` 依 JS 鍵順序的有序陣列。接線時拿掉 `mod.rs` 的 dead_code 放寬。
- 包 5b：`Substituted.private` 是巨集引擎回報「讀到私密來源」的管道，掃描已沿遞迴傳遞。
- 排序一律用 `sort::v8_sort`（不要用 Rust 的 `sort_by`）：比較子非全序時結果會不同。

## 下一步
包 1 結束。換新代理接手包 2（Sonnet）。
