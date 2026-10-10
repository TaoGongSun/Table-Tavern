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
`sortByOrder`（ST `sortFn`）的穩定排序結果：`items` 是 `{ order }`（有限數字，讀取條目時缺欄或不是數字已補 100），`expected` 是排序後的原索引。

## entry-cases.json
- `form: "worldFile"` 照 `fromWorldFile`，`"characterBook"` 照 `fromCharacterBook`：單一條目 → `WiEntry` 欄位＋`decorators`。
- `form: "bookOrder"`：`entries` 是 JSON **原文**（要保住物件鍵的出現順序）→ ST 載入後的條目先後（卡片契約的 key）：陣列形依 `id`（缺則索引）、重複 `id` 後蓋前且位置留在前，物件形整數鍵遞增、其餘照原檔出現順序（`stOrder`）。
- 桌面版另外驗兩件事（Rust 限定，不進 fixture）：每個 `characterBook` 案例經匯入轉成物件形、落檔再讀回要得到同一個 `WiEntry`；`sort-cases` 的條目經同樣轉換後排序結果不變。

## integration-cases.json
巨集×世界書掃描（方案四之 1）：輸入是卡名與公開設定（桌面版的 `public_md`；網頁版把描述、個性、角色深度提示都設成它）、玩家名、聊天（舊到新）、變數、上一輪 outlet、物件形條目（id＝uid）。照 `prompt.ts` 的先後跑卡欄位第一輪 → 掃描（鍵與內文代換、讀上一輪 outlet）→ outlet 換成本輪 → 世界書前／後與注入段落再代換。預期值：`scanned`＝觸發條目掃描時的內文（依 id）、`before`／`after`＝再代換後的前／後組、`injections`＝作者註記與依深度段落（深的在前，同深度 assistant → user → system）、`outlets`＝本輪 outlet、`local`／`global`＝跑完的變數表。chatId 是 `table`、亂數一律 0、沒有上限。網頁版 `web/src/features/chat/integration-parity-runner.ts`，桌面版 `src-tauri/src/world_scan/integration_parity_tests.rs`（擁有文字第一輪 → `scan` → `prepare`）。

## web-save-next-turn.json
網頁存檔端對端（方案四之 4）：網頁版讀 `../web-save/web-export-world-info.json`、玩家再送 `nextUser`，照實送的 `composePrompt` 組下一輪（`web/scripts/web-save-next-turn.ts`）。預期值：`inPrompt`＝內文進了提示的條目（穩定 ID）、`timed`＝掃完的計時表。桌面版（`src-tauri/src/import/web_save/next_turn_tests.rs`）匯入同一份存檔、補同一句玩家句，角色視角掃一次，觸發條目（換成 uid）與落地形狀的計時表要相同。
