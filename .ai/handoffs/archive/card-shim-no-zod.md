> 結案 2026-10-27：squash 進 main，Sol、Grok 驗收第 2 輪 DONE，main 上重跑 verify 12 步綠（接 TestCards）。

# card-shim-no-zod — 卡片 iframe 的 parseMessage 在 zod 桌要跟後端一致

## 結果
- zod 桌的 `Mvu.parseMessage` 交宿主 zod 引擎只算不寫（桌面 `card_vars_parse` → `wiring::card_parse` → `run_batch`；網頁 `TableMvu.parse` → 同一支 zod runner），宿主回 `zod:false` 才在 iframe 跑核心；判定失敗、還沒載好一律 reject。做法、裁決、已知限制見 [plans/card-shim-no-zod.md](../../plans/card-shim-no-zod.md)。
- 真卡 17 條一致性（TestCards 本機限定，CI 略過）：`card-mvu-parse.test.ts` 寫 `TestCards/card-parse-golden.json`，cargo 與網頁讀同一份逐字比。網頁用測試 runner 替身，不是真隔離 iframe＋Worker。三邊 pass 輸出：
  - `✓ card-mvu-parse.test.ts > 真卡：沙盒 parseMessage 與宿主 zod 引擎逐字相同（TestCards 本機限定） > 17 條`
  - `✓ zod-table.test.tsx > 真卡：網頁 zod runner 與桌面宿主逐字相同（TestCards 本機限定） > 17 條`
  - `test mvu_engine::parity_tests::card_parse_cases_match_node_golden_byte_for_byte ... ok`（`--nocapture` 無「略過」）
- 測試通道（2026-10-27）：匯入 HeroTrainingUnderSide_stats，`card_vars_parse` 對 `_.add('World.Invasion', 500)` 得 100、表與基準逐字相同；範例桌回 `zod:false`；不存在的桌回 `detect:` 錯。卡片介面開著時在主框架派 `mvu-parse`（source 用 MessagePort 代替 iframe）經 controller 走完整條，回覆逐字相同、token 不符不回。
- 未驗：真不透明 iframe／WKWebView 內的 shim（排隊、補發事件、逾時）只有 Node 單元測試；實測只到宿主端整條。
