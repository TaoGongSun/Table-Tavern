# 世界書觸發對拍（worldbook-st-trigger-parity）

網頁版（`web/src/features/sillytavern/world-info-*.ts`，照 SillyTavern 06bde939）與桌面版（`src-tauri/src/world_info/`，Rust 逐行移植）跑同一份案例。

- 產生：在 `web/` 底下 `node scripts/gen-world-info-fixtures.mjs`，用網頁版實作跑 `web/scripts/world-info-fixture-cases.ts` 的輸入，寫出本目錄的 JSON。兩邊的測試（`web/src/features/sillytavern/world-info-parity.test.ts`、`src-tauri/src/world_info/parity_tests.rs`）只比對、不改寫。
- 改案例或改任一邊實作導致預期值變動＝重跑產生腳本、人工核對差異，兩邊同一筆 commit。

## scan-cases.json
`checkWorldInfo` 整次掃描。輸入：
- `entries`：`{ id, raw }`，`raw` 是物件形（ST 世界書檔）原始條目，照 `fromWorldFile` 讀、`sortByOrder` 排序後掃描。
- `chat`：新到舊。`settings`：完整設定物件（ST 預設或 MVU 推薦值加覆寫）。`maxContext`：`null`＝沒有上限。
- `globalScan`、`trigger`（預設 `normal`）、`timed`、`random`（依序取用，用完就是案例寫錯）。

固定規則：代換把 `{{user}}`→`Alice`、`{{char}}`→`Bob`，每次輸入記進 `substituteCalls`；計數＝Unicode code point 數；regex 掛勾恆等。

預期值＝網頁版 `WiResult`（`before`、`after`、`examples`、`depth`、`anTop`、`anBottom`、`outlets`、`timed`、`activated`）＋`substituteCalls`＋`randomCalls`（亂數用掉幾個）。`outlets` 寫成依 JS 鍵順序的 `[名稱, 內容[]]` 陣列（要比順序；Rust 讀回 JSON 物件時不保序）。比對時數字以數值比、陣列比順序、其餘物件不看鍵順序。

## regex-cases.json
`parseRegexFromString(key)` 再 `test(haystack)`：`expected` 是 JS 的 `{ parsed, matches }`（`parsed: false`＝當一般字串）。`knownDifference` 是桌面版刻意不同的結果，Rust 測試比它。

## sort-cases.json
`sortByOrder`（ST `sortFn`）在 V8 `Array.prototype.sort` 下的結果：`items` 是 `{ order }`（沒有 `order` 鍵＝undefined），`expected` 是排序後的原索引。order 混了 NaN 時比較子不是全序，結果取決於 V8 的 TimSort，桌面版照它移植。

## entry-cases.json
單一條目 → `WiEntry` 欄位＋`decorators`：`form: "worldFile"` 照 `fromWorldFile`，`"characterBook"` 照 `fromCharacterBook`。
