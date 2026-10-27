# mvu-zod-semantics 規格

zod 卡照上游 mvu_zod，用真 zod 執行卡片自帶的 schema；桌面與網頁共用同一份 JS 的 MVU 引擎。拍板見交接檔 [handoffs/mvu-zod-semantics.md](../handoffs/mvu-zod-semantics.md)。
本檔經 Sol、Grok、主線三方審過；未標來源的細節是施工判斷，標〔模型判斷·未裁決〕。

## 0. 定案
- 桌面宿主用 rquickjs `=0.14.0`（內含 quickjs-ng 0.16.2），預設 allocator，不開 `rust-alloc`〔三方共識，依 QuickJS 短驗證〕。短驗證證據：mathjs、json5、yaml、jsonrepair、zod 4.6.7 原樣跑通；三份卡片 schema 的輸出與 Node 逐字相同；記憶體、時間、堆疊上限實測生效；沒有網路與檔案 API；觸限後 runtime 可能壞掉，所以觸限就重建。
- 新依賴：npm `zod`、crate `rquickjs`；網頁另加 `es-module-lexer`。三者都是 MIT。
- 網頁版先用 Worker，只有時間上限；記憶體硬上限尚未符合裁決要求，記在 BACKLOG〔作者裁決 2026-10-27〕。
- 整批 5 秒期限不放寬〔三方共識〕。大表（100 千字元級）每條約 25 ms，5 秒約可跑 200 條。
- 同一桌有兩張卡都呼叫 `registerMvuSchema`：用先匯入的那份；第二份拒絕，並明白告知玩家〔模型判斷·未裁決，主線裁〕。
- schema 異常情況要各自明定行為，不得悄悄降級成非 zod 桌：沒有註冊、腳本停用、載入失敗〔模型判斷·未裁決，主線裁〕。

## 1. 共用核心（JS，一份供三個宿主）
三個宿主：卡片 iframe（`card-mvu-shim-source.ts`）、網頁宿主（`web/mvu-engine-plugin.ts` → `web/src/features/mvu/engine.ts`）、桌面 QuickJS。

### 1.1 宿主介面
`createMvuCore(env)`，型別見 `src/features/mvu-engine/host-contract.ts`：
```
env.host.evaluate(op, text, done) -> 取消函式 | null   // done({ok:true, value} | {ok:false, error})，可同步呼叫
env.host.timers?: { set(fn, ms), clear(id) }          // 選配、成對：有就設值解析保險逾時
env.host.utf8Length(text) -> number
env.host.macros() -> { user, char } | null
env.emit(name, args, isolate) -> Promise              // MVU 事件
env.events、env.lodash
```
- iframe 與網頁宿主：`evaluate` 送 Worker（單筆 200 ms 逾時），給 timers；保險在核心，逾時呼叫取消函式並以 eval-timeout 整次失敗。
- 純邏輯不變：schema、路徑、指令抽取、套用、上限。
- 片段規則照舊：不得出現反引號與「錢字號加左大括號」。

### 1.2 檔案落點
核心升格成 `src/features/mvu-engine/`。理由：三個宿主真的在用，而且它屬於 MVU 引擎本身，不屬於 card-interface。

| 檔 | 內容 |
|---|---|
| `engine-source.ts` | 依序串接片段，對外 `buildMvuEngineSource()` |
| `source-limits.ts` | 上限、值解析橋接（改走 host） |
| `source-schema.ts` | generateSchema／cleanUpMetadata／applyTemplate |
| `source-path.ts` | 路徑修正 |
| `source-commands.ts` | 指令抽取 |
| `source-apply.ts` | set／insert／delete／add |
| `source-update.ts` | updateVariables、結果上限、parseMessage |
| `source-zod.ts` | P2 新增 |
| `value-parser.ts` | 原 `card-mvu-parse-engine.ts` |
| `host-contract.ts` | host 介面與回傳契約型別 |
| `quickjs-entry.ts` | P3 新增，桌面 bundle 入口 |

`card-interface/mvu/` 留 shim、eval-host、eval worker、write。

### 1.3 zod 迴圈（P2）
- 掛在 `COMMAND_PARSED_for_zod`／`_ended_for_zod` 的位置，照 mvu_zod 逐條處理：深拷貝 → 套用 → 整表 safeParse → 成功就取代，失敗整條丟掉。最後清空指令陣列，MagVarUpdate 那一段就不再跑。
- 開局：initvar 先 `cleanUpMetadata` 再 safeParse；成功合回，失敗照原值並回報。
- zod 桌照上游不做 `$meta` 檢查。網頁 `initializeSchema` 對 zod 卡要跳過。

### 1.4 回傳契約（P2 定版）
```
{ table, results: [{index, op, path, status: applied|no-change|rejected|ignored, reason?, issues?}], changes: [{path, before, after}] }
```
- 單條被 schema 拒收：同批其他成功的指令照樣保留。
- 整次失敗（逾時、觸限、例外）：丟錯，不回部分結果。
- table、results、issues、changes 各自設大小上限。
- 巢狀 `z.object` 靜默剝掉的鍵會變成 no-change，帳本要把它當拒收回饋，免得 GM 以為已寫入。
- 「只驗證」API 若在 P4 加上，網頁版同一輪補上。

### 1.5 schema 載入
- 匯入時從 tavern_helper 腳本找出含 `registerMvuSchema` 的那一支，存原文。選擇規則在 P2 定版。
- 執行環境只放 `z`、`_`、`$`。
- QuickJS：loader 白名單只把 mvu_zod 的網址對應到墊片模組。

## 2. 桌面 Rust 宿主（P3、P4）

### 2.1 `src-tauri/src/mvu_engine/`
四個檔各管一件事：`host.rs` 執行緒與通道、`runtime.rs` 上限／loader／bundle、`contract.rs` 輸入輸出型別、`tests.rs` 測試；`mod.rs` 對外。
- 測試要涵蓋：記憶體炸彈、逾時、無限遞迴、import 白名單、三卡對照。

### 2.2 執行緒與生命週期
- runtime 不是 `Send`：開一條專用 OS 執行緒（stack 8 MB），收 mpsc 請求，用 oneshot 回覆，外層逾時 6 秒。
- 每桌一個 runtime，最多 2 桌，LRU 淘汰。
- 以下情況重建 runtime：觸限、例外、schema 雜湊變了、閒置 10 分鐘。
- 上限：記憶體 64 MB、整批 5 秒、堆疊 1 MB。
- 單一 monotonic deadline 涵蓋整次處理：schema 載入、解析、逐條驗證、後處理後的第二次驗證、輸出；第二次驗證不重新起算 5 秒。
- 逾時復原：外層逾時一到，就把執行緒標成作廢、改開新的接手。未回收的作廢執行緒有數量上限，重建也有次數上限，超過就整次失敗並回饋，不可無限作廢再開。
- panic 用 `catch_unwind` 包住，轉成整次失敗。
- 輸入在進引擎前先照 `PARSE_LIMITS` 擋大小。
- QuickJS 宿主不給 timers，`evaluate` 在同一個 context 直接呼叫 `runEval`，每次都必須回覆（呼叫 done）或拋錯：
  - interrupt 中斷要在呼叫 done 之前拋出；包成 `{ok:false}` 會變成單條失敗並繼續跑，done 之後再拋會被吃掉。
  - interrupt 無法替閒置中的未結算 Promise 產生回覆；宿主不得留下等外部事件才結算的請求。
  - 執行環境要自備 `TextEncoder`（`value-parser.ts` 直接 `new TextEncoder()`）。

### 2.3 bundle
- npm script `build:mvu-bundle` 產生 `src-tauri/gen/mvu-engine.js`（gitignore）。
- verify 的 cargo 步驟前、tauri 的 before 指令都先跑它。
- `build.rs` 檢查檔案存在；缺了就停下，提示要先跑哪個指令。

### 2.4 提交契約
現況鏈：`TurnGuard::commit`（chat.rs:772）→ `apply_gm_block`（write.rs:451）→ `gm_commit_apply`（chat.rs:390）→ `apply_block_typed`（ledger.rs:247）。

zod 桌改成三段：
1. **固定輸入（短鎖）**：`message_vars::turn_input` 沿用 `apply_gm_block` 開頭那組核對。`ensure_active` 物化出的 input 先存回回合紀錄，再回傳 `input_rev`。schema 雜湊與 macros 一起固定；若不固定，就列入失效條件。
2. **鎖外跑引擎**：`run_batch` 回 1.4 的契約。
3. **短鎖重驗才寫**：核對 turn_id、phase、scene、generation、input_rev、schema 雜湊，不符就回 None。
   - pending 取這次的最終表，也就是含後處理的結果。新表照樣經 `merge_tree_change_typed`（或等價的合成），重擲與旗標才會進落檔的表；不能直接拿 `result.table` 跳過合成。

後處理〔Sol、Grok 意見合併〕：
- zod 桌不套 `[mvu_update]` 的夾值。重擲、觸發、衍生照跑：帶 initvar 的未重構桌 `incremental` 為真。
- 後處理寫進樹之後，在鎖外再送引擎驗證一次。
- 以下任一情況，退回引擎原表並回饋：
  - 第二次驗證不過；
  - transform 改動了值；
  - 巢狀剝鍵，導致剛寫的值對不上。
- transform 不保證冪等。第二次驗證的語意是「把後處理結果再過一次 schema，看結果是否與寫入值相同」，不得稱為純驗證。
- 退回時，triggers、旗標、records 一併撤回（ledger.rs:306 同時改這些），不能只退變數表。

其餘條款：
- `block.fields` 與清單整支替換在引擎路徑跑不跑，P4 開工時寫明。
- 帳本：新檔 `mechanism/engine_records.rs`，把逐條結果映射成 `Record`，回饋句沿用 `build_notes`。`_` 路徑的回饋留給 P6。
- 開場白走同一條路（transcript.rs:389 → `opening_table`）。整次失敗時照 `state_error` 留正文、不裝變數，不走「世代不符回 None」。
- initvar 的 safeParse 在建 `init_source` 時做一次；`scene_seed_for_next` 不重跑。
- 收回讀已存快照，不重跑 transform。
- 整次失敗：狀態整批不提交，正文照常落檔，回饋「變數更新失敗，可能需要重 Roll」。

### 2.5 判定
- zod 桌＝未重構桌（Upstream），且有卡註冊 schema。
- `CardInterface.mvu_schema` 存腳本原文，偵測寫在新檔 `import/card_schema.rs`。
- 重構桌照舊。非 zod 的 MVU 桌本案照舊走 Rust；等 mvu-command-syntax／mvu-meta-schema 接上引擎後，再清掉只服務 Upstream 的 Rust 碼。

### 2.6 文件例外
`evaluator.rs` 開頭、`import/ejs.rs`、card-mvu-shim 計畫裡「不執行卡片腳本」的紅線，加上例外：只限卡片變數 schema、只在隔離環境執行。在功能啟用的那一包同步改。

## 3. 分包與驗收
| 包 | 內容 | 驗收要點 |
|---|---|---|
| P1 | 共用核心抽出＋host 介面，行為不變 | verify 綠；三宿主的行為與事件順序要逐項比對，不能只拿「測試數量沒變」代替 |
| P2 | zod 迴圈、schema 載入、回傳契約與 schema 選擇規則定版 | 見下方 P2 對照清單 |
| P3 | Rust QuickJS 宿主 | 短驗證全部轉成 cargo 測試；deadline、逾時復原上限 |
| P4 | 桌面接線 | 過期結果不提交；失敗時正文仍落檔；開場白同路；收回不重跑 transform；後處理退回連帶撤回 |
| P5 | 網頁宿主（P2 契約凍結後，與 P3／P4 並行） | 見下方 P5 驗收清單 |
| P6 | 裁決落地 | parity json 加 zod 分支、zod 回饋句、`_` 路徑回饋 |

P2 對照清單：上游獨立對照，逐項比對：
- insert 遇 nil 時，建物件或陣列；
- move 陣列；
- 指令別名；
- JSONPointer；
- 拒收之後的後續指令；
- 非冪等 transform。

Node 與 QuickJS 結果相同，只證明兩個宿主一致，不能當成和上游一致。

P5 驗收清單：
- 先驗 `script-src 'self'` 與 blob 模組相容；
- lexer 涵蓋動態 import，含非字面量 specifier；
- 真瀏覽器測試：schema 與墊片能載入，其他模組與網路被擋。

順序：P1 → P2 → P3 → P4，P6 最後。

## 4. 網頁宿主
- schema 與 zod 迴圈在 module Worker 裡跑。該路徑由 `site-headers.ts` 給自己的 CSP；開機時刪掉網路 API，只當第二道防線。
- 卡片的 jsdelivr import 用 es-module-lexer 改寫，指向墊片的 blob URL；其他 import 一律拒絕。
- 期限到了就 terminate Worker。網頁沒有記憶體硬上限（見 0 節）。

## 5. 結構
- 已超過 1000 行的檔：parse-source 1047 行，P1 拆；`commands/chat.rs` 1486 行與 `data/scene/transcript.rs` 1248 行，本案只加呼叫，拆檔另立案。
- 接近上限的檔，新責任放新檔：`ledger.rs` 876 行 → `engine_records.rs`；`import/interface.rs` 933 行 → `card_schema.rs`。
