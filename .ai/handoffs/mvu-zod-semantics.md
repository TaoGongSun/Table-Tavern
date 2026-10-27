# zod 版 MVU 卡的更新語意改以 mvu_zod 為基準

Status: in-progress（分支 mvu-zod-semantics）

## Summary
mvu-array-index-path 驗收時查到：zod 版 MVU 卡（registerMvuSchema，TestCards 帶 [initvar] 的四張全是）實際由 StageDog/tavern_resource 的 util/mvu_zod.ts 在 `mag_command_parsed_for_zod` 自行 applyCommand 並清空指令，MagVarUpdate 的 applySet／applyInsert／applyDelete／applyAdd 一條都收不到。本地 mvu-replace-numeric、mechanism-error-feedback、mvu-array-index-path 都以 MagVarUpdate 為基準。mvu_zod 差異：set 直接 `_.set`（不做 VWD 第 0 項、不轉 Number、路徑不存在照建）；add 只收數字、無 toPrecision(12)、無 VWD；insert 容器 nil 先試 {} 再 []、頂層不查父路徑；delete 陣列用 pullAt；move 會搬（delete 再 set）；`_` 開頭路徑靜默忽略；每條指令最後過卡片 zod schema（safeParse，含轉型、夾值、預設值），不通過整條不套用。

## 拍板（2026-10-27，Sol／Grok 兩輪討論後）
- zod 卡照上游 mvu_zod 行為做，以真 zod 執行卡片自帶 schema，公開前納入〔作者裁決 2026-10-27，修訂 2026-10-12「公開前維持現行基準」〕。
- 卡片自帶的變數 schema 准在隔離環境執行（無網路、無檔案存取、有時間與記憶體上限）；其餘卡片腳本照舊不執行〔作者裁決 2026-10-27〕。
- mvu-replace-numeric 的〔作者裁決 2026-10-04：不夾值、轉不成數字寫 null〕縮小為只管沒重構、非 zod 的卡；zod 卡照上游：夾值、轉不成數字整條拒收並保留舊值；重構桌照現行規則〔作者裁決 2026-10-27〕。
- 桌面與網頁共用同一份 MVU 引擎（不在 Rust 另寫第三份）。宿主先短驗證再定：先抽出無 DOM 的共用核心，再實測 QuickJS 能否跑 mathjs、zod 並守住記憶體上限；能就桌面用 QuickJS，不能就用 Worker 加可驗證的隔離方案〔作者裁決 2026-10-27〕。
- 提交契約（兩位審查方共識）：Rust 固定輸入→鎖外呼叫引擎→短鎖內重驗回合／幕／世代／輸入版本才寫；引擎回傳完整變數表＋逐條結果＋變動，Rust 不再二次套用原指令；帳本由逐條結果映射；開場白同一路徑；收回讀已存快照、不重跑 transform；單條 schema 拒收保留同批其他成功指令，整次執行失敗整批不提交並回饋；後處理（重擲、觸發、衍生）對 zod 已驗資料的規則要先明定。
- mvu-command-syntax、mvu-meta-schema 改成「接上共用引擎」，排在共用引擎之後；實卡缺，用乾淨測試卡＋DongeonMaster initvar 去 zod 腳本的衍生測試卡（不算原生實卡驗收）。

## 實作期裁決（2026-10-27，作者經主線提問工具）
- 上游對照放行自動比對：本機執行上游 `mvu_zod.ts`，不入版控（PolyForm Noncommercial）〔作者裁決 2026-10-27〕。
- zod 指令值的物件／陣列字面量用 JSON5，不用上游的 new Function〔作者裁決 2026-10-27〕。
- MagVarUpdate 套用路徑的原型汙染在本案修〔作者裁決 2026-10-27〕。
- zod 路偏離上游、路徑只走自有屬性〔作者裁決 2026-10-27〕。
- 網頁版先用 Worker，只有時間上限，記憶體硬上限另記 BACKLOG（主線立案）〔作者裁決 2026-10-27〕。

## 規格
[plans/mvu-zod-semantics.md](../plans/mvu-zod-semantics.md)：宿主 QuickJS（rquickjs `=0.14.0`）、分包 P1–P6、各包驗收。

## 現況（分支 mvu-zod-semantics，worktree 在主 checkout 之外）
- P1 已收（三方共識）：MVU 共用核心搬到 `src/features/mvu-engine/`，宿主差異走 `createMvuCore(env)`。iframe 與網頁宿主的行為、事件順序和 main 逐項相同。commit 79ae26e7、20452d01。
- P2 已收（第 4 輪 Sol、Grok 都說可收，三方共識）：
  - `source-zod.ts` 照上游 mvu_zod 實作，`parseMessageDetailed` 回傳契約與 schema 選擇規則定版（規格 §1.3–1.6）。
  - 共用路徑存取只走自有屬性（`source-path.ts`），MagVarUpdate 與 zod 都用。
  - commit a0b39e32、aa7b60cf、3f2c2147、6b48f359、9e00c47e、b313efb5。
- 根 package.json 已加 `zod` 4.6.7（鎖版）。web 端到 P5 才加。
- P3 第 2 輪（Grok 可收、Sol 一項必改）已修：全域名額改成「執行中＋作廢未結束」，送出前取得。之前第 1 輪的修正：被淘汰的執行中 worker 逾時照樣作廢計數、回成功前重驗期限、鍵順序保序（serde_json preserve_order）、中斷穿過 catch／finally 的 cargo 測試、重開上限改每桌。三方共識定案已寫進規格 1.3、1.5、2.2、2.4a。宿主在 `src-tauri/src/mvu_engine/`，入口 `quickjs-entry.ts`，bundle 由 `npm run build` 產生。

## Next action
- 等 P3 第 3 輪驗收；過了才開 P4（桌面接線）：讀規格 2.4–2.6 與 3 節 P4 列，特別是 2.4a 的存表保序要求。`lib.rs` 的 `mvu_engine` 目前 `allow(dead_code, unused_imports)`，接線時拿掉。2.6 的文件紅線例外在 P4 同步改。
- 發版前要驗 Windows MSVC 編譯（規格 0 節）。

## 本機驗證
- 上游 zod 對照：
  - 位置：`scratchpad/zod-oracle/oracle.ts`。
  - 跑法：在該目錄用 worktree 的 esbuild 打包，參數 `--alias:@util/common=./util-common-stub.ts`，外部套件 lodash、yaml、zod、klona、mathjs、json5、jsonrepair。`node_modules` 裡 klona 由 npm 安裝，其餘 symlink 到 worktree 的 `node_modules`。最後 `node oracle.mjs`。
  - 結果：印出每案 SAME／DIFF 與 `diffs N of 41`，上游預期值寫到 `scratchpad/zod-oracle/upstream-expected.json`。
  - 上游來源：StageDog/tavern_resource `util/mvu_zod.ts` 29becf3，不得進 repo。
  - scratchpad 不在時，照同樣做法重建。
- 真卡測試：`source-zod.test.ts` 的三張真卡（DongeonMaster、bcd368、HeroTraining）只在本機有 `TestCards/` 時跑，CI 略過。worktree 沒有 TestCards，要跑就暫時 symlink 主 checkout 的 `TestCards`，commit 前刪掉。
- QuickJS 對照：vitest（`quickjs-entry.test.ts`）有 TestCards 時把 Node 跑三張真卡的結果寫到 `TestCards/zod-cards-golden.json`，cargo 的 `real_cards_match_node_golden` 讀它比 QuickJS（沒有就略過），所以先跑 vitest 再跑 cargo。
- 舊行為對照：`scratchpad/p1-diff/diff.ts`，main 對分支 42 項 iframe／網頁宿主行為與事件順序。動到共用核心就重跑：worktree 的 esbuild 帶 `NODE_PATH=<worktree>/node_modules` 打包成 ESM（加 createRequire banner），`node diff.mjs <輸出目錄>`。

## 合併時處理
- 舊檔名連結還沒修：`card-mvu-parse-source`／`card-mvu-parse-engine` 已改成 `mvu-engine/engine-source`、`mvu-engine/value-parser`。
  - 仍寫舊名的檔：`.ai/plans/card-mvu-shim.md`（4 處）、`.ai/handoffs/card-mvu-shim.md`（1 處）、`.ai/plans/mechanism-error-feedback.md`（2 處）、`.ai/plans/web-version.md`（1 處）。
  - `.ai/handoffs/ai-card-refactor.md:97` 連到 `tasks/mvu-zod-semantics.md`，檔案已搬到 `handoffs/`，連結斷了。
- 討論紀錄：Sol 串 01a17baf-eea3-7561-9b5a-91e70855970b、Grok 串 01a17baf-eacf-7651-b1b8-6a502d1e7400；P2 驗收 Sol 01a17bed-bc57…、Grok 01a17bed-bbec…。
