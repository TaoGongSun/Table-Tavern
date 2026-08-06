# Handoff: card-import-flow

## Current state
四包＋後續九項修正全部實作完成、逐項使用者實機驗收通過、逐包 commit。2026-08-06 收工時無未完成的實作，也無已知缺陷。
剩最初驗收清單的第 4、7 項還沒跑過——當初卡在沒有樣本卡，現在 `TestCards/` 的卡足夠了（配方見 Next action）。

## Completed
全部已驗收，明細與證據見 [archive/card-import-flow-completed.md](archive/card-import-flow-completed.md)。

## Verification
- 最終自驗（2026-08-06 收工）：cargo test 327 綠、npm run build ✓、vitest 23 綠、check:i18n 九語系 OK。
- commit：548905a（包 1）、1939af6（包 2）、d8abe8b（包 3）、b4acab7（包 4）、a73ca94（雙世界書可融合）、26f6def（身分判定改版三步）、83a9e3a（條目保險）、d6688fb（角色卡開場白）。

## Remaining
最初的實機驗收清單只剩兩項沒跑：

4. **「復原上次匯入」連按逐筆退** —— 單筆退整筆（角色＋條目＋桌名）、玩家改過的條目保留並提示保留數，也一併沒驗。
7. **有角色卡的桌補匯配套世界書 → 不跳框直接進**（companion 路徑）。

## Next action
兩項的驗收配方（素材都在 `TestCards/`，不必再等）：

- **第 7 項**：空桌 → 匯 `塞拉菲·内藤.png`（角色卡，`character_book` 空、備用開場白 0，所以桌上世界書條目維持 0）→ 再匯 `main_Furry_Anthro Anatomy_world_info.json`。預期：**不跳任何框**直接匯進這桌（`decideImportRoute("worldbook", true, ["character"], false)` → `companion`）。
- **第 4 項**：同一桌疊兩筆以上匯入紀錄（匯 A → 身分框選角色卡；再匯 B → 身分框 → 路由框選「匯進這桌」），然後連按兩次「復原上次匯入」，看是否逐筆倒退而不是一次清光。先手動改過其中一條世界書條目的內容，驗「改過的保留並提示保留數」。

兩項驗完即可結案（狀態改 completed、TASKS.md 那行搬 DONE.md）。

**已知待觀察（非缺陷）**：有匯入紀錄的桌再匯卡會連跳兩個框——先問身分，再問哪一桌；空桌只有一個框。這是「所有卡都讓玩家選身分」的直接後果，實機覺得煩才把兩框併一個。

## Constraints
- AI 卡重構按鈕另案（[ai-card-refactor](../tasks/ai-card-refactor.md)）；入口方向已拍進該任務檔。
- 新 UI 字串十語系逐鍵（zh-TW 正典）；驗證四件套：cargo test＋npm run build＋npm test＋check:i18n。
- **收據為主，條目最多當保險**（2026-08-06 使用者立規）：路由判準以匯入收據為準，不為測試期間才有的舊桌問題改寫長期標準。
