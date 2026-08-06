# Handoff: card-import-flow

## Current state
四包＋後續十項修正全部實作完成，最初的實機驗收清單七項**全數通過**。2026-08-06 收工時無未完成的實作，也無已知缺陷。
驗收第 7 項通過的同時暴露出 companion 路徑的前提是假的，當場拍板拿掉（見下）。任務可結案。

## Completed
全部已驗收，明細與證據見 [archive/card-import-flow-completed.md](archive/card-import-flow-completed.md)。

### companion 路徑刪除（2026-08-06 驗收第 7 項當場拍板，推翻拍板 4 的配套例外）
第 7 項「有角色卡的桌補匯配套世界書 → 不跳框直接進」照配方跑過了，但使用者當場指出：拿來驗的 `塞拉菲·内藤` 與 `Anthro Anatomy` **根本不配套**，卻一樣走了零打擾路徑。原因是 `isPureWorldbookFile` 只知道「這是不是一份純世界書檔」，講不出它跟桌上那張卡有沒有關係——**配套與否只有玩家知道，程式偵測不出來**。
拍板：拿掉 `companion`，這桌一旦匯過東西就一律開框讓玩家自己決定。連帶刪掉只服務它的 `isPureWorldbookFile` 參數（`decideImportRoute` 與 `routeImport` 各一個）與 `ImportRoute` 的 `"companion"` 成員。決策表剩四格：收據為空→direct（世界書身分且桌上有條目→merge_worldbook 保險）／收據有 worldbook 筆＋世界書身分→merge_worldbook／其餘一律 ask。

## Verification
- 最終自驗（2026-08-06 收工）：cargo test 327 綠、npm run build ✓、vitest 22 綠、check:i18n 九語系 OK；殘留檢查 grep `companion|isPureWorldbookFile` = 0。
- 最初的實機驗收清單七項**全數通過**（1／2／3／5／6 於 2026-08-05，4／7 於 2026-08-06）。
- commit：548905a（包 1）、1939af6（包 2）、d8abe8b（包 3）、b4acab7（包 4）、a73ca94（雙世界書可融合）、26f6def（身分判定改版三步）、83a9e3a（條目保險）、d6688fb（角色卡開場白）、6c8db5e（交接檔瘦身）。

## Remaining
無。

## Next action
結案：狀態改 completed、從 TASKS.md「In progress」移出、那一行搬 [DONE.md](../DONE.md)。

**已知待觀察（非缺陷）**：有匯入紀錄的桌再匯卡會連跳兩個框——先問身分，再問哪一桌；空桌只有一個框。這是「所有卡都讓玩家選身分」的直接後果，實機覺得煩才把兩框併一個。

## Constraints
- AI 卡重構按鈕另案（[ai-card-refactor](../tasks/ai-card-refactor.md)）；入口方向已拍進該任務檔。
- 新 UI 字串十語系逐鍵（zh-TW 正典）；驗證四件套：cargo test＋npm run build＋npm test＋check:i18n。
- **收據為主，條目最多當保險**（2026-08-06 使用者立規）：路由判準以匯入收據為準，不為測試期間才有的舊桌問題改寫長期標準。
