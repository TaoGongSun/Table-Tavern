# initvar-nonzod-overwrite：桌面非 zod 桌開場白 `<initvar>` 整份取代初始值

## 上游行為（MagVarUpdate `initvar/variable_init.ts` initCheck，只當規格書讀）
- 非 zod 卡與 zod 卡同一條：第 0 樓每個 swipe 用 `/<(initvar)>(?:\s*```.*)?([\s\S]*?)(?:```\s*)?<\/\1>/gim` 抓區塊，逐塊 `parseString(substitudeMacros(...))` 深合併；解析失敗的區塊記錯略過；至少一塊成功就 `stat_data` **整份取代**世界書 `[initvar]` 合併出的值，再重跑角色書以外世界書的 initvar（補缺，不蓋覆寫值），然後套開場白本身的指令。
- 只在開局（`getLastMessageId()==0`、還沒初始化）發生；非 zod 不對覆寫值做 cleanUpMetadata、也不重生 schema。

## 重現
合成非 zod MVU 卡（MVU 腳本、停用的 `[initvar]` 條目 `角色: {金幣: 10, 地點: 村莊}`），開場白帶
`<initvar>```yaml 角色: {金幣: 999, 職業: 騎士}```</initvar>`，走 `data::append_opening`：
開場事件 stat_data 仍是 `{"角色":{"地點":"村莊","金幣":10}}`，應為 `{"角色":{"金幣":999,"職業":"騎士"}}`。
另驗：桌面 Rust 的 `parse_initvar_tree` 吃不了 JSON 體（整段變一個鍵）、`999 # 註解` 留成字串——不能拿它解開場白區塊。

## 根因
- `src-tauri/src/data/scene/transcript.rs:377-380`：非 zod 分支只 `ensure_active(tx, Some(&macros))`，`raw` 沒傳進建種子那一步。
- `src-tauri/src/data/message_vars/mode.rs:66-108`：`ensure_active` → `ensure_active_seeded(.., None)` 一律由狀態樹 `materialize`，整條非 zod 路徑沒有任何地方讀開場白的 `<initvar>`。
- 對照 zod：`mvu_engine/wiring.rs:510` 把 `raw` 交給 `seed()` → QuickJS `quickjs-entry.ts` seed 的 `openingOverride(opening) ?? stat`。
- 與 initvar-entry-yaml-semantics 同根：桌面非 zod 的 initvar 全走 Rust 行剖析器，上游語意的 `parseStructured` 只在共用核心（QuickJS）裡。本案讓開場白區塊走共用核心；那案日後把條目原文也送同一個 `Op::Seed`（`initvars_json` 換條目、`base_json` 換 null）即可對齊，範圍不擴。

## 做法（送審第 1 輪 Sol、Grok 同意〔模型判斷·三方共識〕）
1. **重用 zod 的 `Op::Seed`，不修 Rust 剖析器**：桌面非 zod 開場白在鎖外先算覆寫值，呼叫 `shared().seed(world_id, 無 schema, SeedBase::Tree(樹物化的 stat), raw, macros, deadline)`。沒有註冊 schema 時，`zodInitialize` 回 `{ok:true, zod:false}`（`source-zod.ts:300`），所以只做 `openingOverride ?? base` → cleanUpMetadata。
2. **套用判定要明確**：`quickjs-entry.ts` 的 seed 回傳多帶 `applied`（`openingOverride` 不是 null），`InitOutput.applied` 接住；zod 路徑忽略它。不用「結果與底相等」或鍵序比較代替。
3. **元資料**：cleanUpMetadata（刪各層 `$meta`、陣列裡的擴充標記與 `$arrayMeta` 載體元素）與既有非 zod 樹物化種子走的 `convert::clean` 刪的是同一組（查證見下節），新路徑不多清也不少清。
4. **引擎無 schema runtime**：`SchemaScript::plain()` 帶明確旗標（不靠空字串；zod 卡的空腳本照舊報「沒有呼叫 registerMvuSchema」）；`runtime.rs` build 遇到它跳過卡片模組與註冊檢查；runtime 快取身分＝旗標＋雜湊（`host.rs` process 不能只比雜湊）。
5. **預篩，不符就不啟動引擎**：不是 zod 桌、`raw` 含 `<initvar>`（不分大小寫）、這一幕還沒建種子、這桌符合卡片變數模式，四者都成立才呼叫。`{{char}}`／`{{user}}` 用實際建種子依據（`seed_basis`）算出的有效巨集。新邏輯放新檔 `mvu_engine/plain_opening.rs`。
6. **鎖內**：`transcript.rs` 非 zod 分支呼叫 `message_vars::ensure_active_overridden`，它明確回報這次有沒有發布覆寫種子（`ensure_active_seeded` 已啟用也回 true，不能只看它）。發布時，種子＝提交當下樹物化的表把 stat_data 換成覆寫值；並在取 `before` 快照與套開場白指令之前，把 `world.state.tree` 換成新種子的投影。這一幕已啟用（別的寫入搶先）就丟掉覆寫，照現狀。
7. **失敗退回**：沒有套用，或引擎逾時、超過 2 MB、載入失敗，就照現狀以提交當下的樹物化種子啟用。失敗時開場白 `outcome.records` 多一筆 Error 帳本「開場白的 `<initvar>` 沒能套用：原因」，隨開場白既有的帳本寫入落檔。
   - **引擎忙（Busy）先重試**：全機同時在跑的請求（含作廢未結束）達上限 2（`host.rs:242-247`），或本桌在時間窗內作廢重開達上限（`host.rs:322-326`），會回 Busy。這時在這次開場的同一個期限內每 50 ms 再試一次，不占資料鎖、等待時不握名額；期限內仍 Busy 才退回並記帳。閒置的 runtime 留在 LRU 池裡只占池位，不算 Busy。理由〔模型判斷·主線裁決〕：非 zod 一旦用世界書值發布種子就不會重套、收不回，而鎖外重試成本低。
   - **zod 判定讀檔失敗**：開場照舊物化（zod 路徑同樣把它當非 zod 桌），記一筆「讀不到這桌的卡片判定」帳後退回，不靜默丟掉覆寫。
8. **網頁版不改程式**：`web/src/features/mvu/runtime.ts:75-79` 非 zod 已照上游覆寫；只補鎖定覆寫的測試。

## 元資料查證
既有非 zod 種子：`[initvar]` → `parse_initvar_tree` → `mode.rs materialize` → `convert::tree_to_stat` → `convert::clean`（`convert.rs:168`），刪各層 `$meta` 鍵、陣列裡的 `$__META_EXTENSIBLE__$` 與 `$arrayMeta: true` 且帶 `$meta` 的元素。共用核心 `cleanUpMetadata`（`source-schema.ts:136`）刪的是同一組，所以新路徑與舊種子一致。

## 已知限制〔模型判斷·未裁決〕
- 覆寫只在這一幕第一次建種子時套（同 zod 與上游）；之後再貼另一則開場白不會重套。
- 上游覆寫後會重跑「角色書以外世界書」的 initvar 補缺；桌面在開場白之前就匯入的額外世界書 initvar 已混進狀態樹，會被覆寫一起蓋掉（開場白之後才匯入的照舊補缺）。補回併進 initvar-entry-yaml-semantics，桌面網頁一起修〔作者裁決 2026-10-27〕。
- 上游非 zod 不清覆寫值的元資料；桌面一律清（與既有種子、zod 路徑一致）。
- 開場白選「翻譯版」貼上時（`commands/scene.rs` `translated`），正文與 `<initvar>` 都取翻譯後文字，區塊可能被翻壞；zod 路徑同樣如此。一般貼上取原卡原文求值巨集，`<initvar>` 完整到達 `append_opening_keyed`（已追過）。
- 非 zod 桌 seed 一次後，引擎 worker 留在 LRU 池占一個池位，直到被淘汰（不影響 Busy 名額）。

## 驗收
- Rust 整合測試（新檔 `import/tests/initvar_opening.rs`）：重現案例（值與鍵序 `金幣`→`職業`、狀態樹投影一致）；JSON5 體；多塊合併；混合成功／失敗區塊；全部失敗且底含 `$meta`→原種子；成功但同值、鍵序不同也算套用；空物件；`{{user}}`／`{{char}}` 代換；`999 # 註解` 解成數字；`<INITVAR>` 大寫；已啟用不重套；開場白自帶指令以覆寫值為底；引擎整次失敗退回＋帳本；Busy 期限內重試成功；期限內一直 Busy 退回＋帳本；zod 判定讀檔失敗退回＋帳本。
- 引擎測試：無 schema runtime 能 seed、回 applied；zod 卡空腳本照舊報載入失敗；同桌在 zod 與無 schema 間切換會重建；LRU 淘汰後 zod runtime 正常重建。
- web 測試：非 zod 開場白 `<initvar>` 覆寫。
- `npm run verify` 全綠；測試通道用手改的非 zod MVU 卡（`TestCards/clean-hero-roster.json` 複本、開場白加 `<initvar>`；DongeonMaster、HeroTrainingUnderSide_stats 有啟用中的 `registerMvuSchema`，在本 app 是 zod 桌，不能拿來驗）看狀態面板是覆寫值。

## 改動檔案（行數，前 → 後）
| 檔案 | 行數 |
|---|---|
| `src-tauri/src/data/scene/transcript.rs` | 1279 → 1297（只加呼叫與樹同步） |
| `src-tauri/src/data/message_vars/mode.rs` | 194 → 221 |
| `src-tauri/src/data/message_vars/mod.rs` | 114 → 115 |
| `src-tauri/src/mvu_engine/plain_opening.rs`（新） | 83 |
| `src-tauri/src/mvu_engine/contract.rs` | 196 → 211 |
| `src-tauri/src/mvu_engine/host.rs` | 595 → 596 |
| `src-tauri/src/mvu_engine/runtime.rs` | 370 → 377 |
| `src-tauri/src/mvu_engine/mod.rs` | 21 → 26（含測試共用的引擎排隊鎖） |
| `src-tauri/src/mvu_engine/wiring.rs` | 574 → 574 |
| `src-tauri/src/mvu_engine/wiring_tests.rs` | 793 → 790（排隊鎖改用共用的） |
| `src-tauri/src/mvu_engine/tests.rs` | 871 → 957 |
| `src-tauri/src/import/tests/initvar_opening.rs`（新） | 200 |
| `src-tauri/src/import/tests/mod.rs` | 10 → 11 |
| `src/features/mvu-engine/quickjs-entry.ts` | 169 → 172 |
| `src/features/mvu-engine/quickjs-entry.test.ts` | 174 → 184 |
| `web/src/features/mvu/initvar-opening.test.ts`（新） | 40 |

不碰 `transport/response.rs` 與 card-interface。
