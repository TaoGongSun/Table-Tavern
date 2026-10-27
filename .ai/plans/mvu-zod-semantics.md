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
| `source-zod.ts` | zod 迴圈、開局驗證、變動清單 |
| `value-parser.ts` | 原 `card-mvu-parse-engine.ts` |
| `host-contract.ts` | host 介面與回傳契約型別 |
| `quickjs-entry.ts` | P3 新增，桌面 bundle 入口 |

`card-interface/mvu/` 留 shim、eval-host、eval worker、write。

### 1.3 zod 迴圈（`source-zod.ts`）
- 照上游 `util/mvu_zod.ts`（29becf3）。核心有 `env.z` 且已註冊 schema 才走 zod：
  - 逐條處理：深拷貝 → 套用 → 整表 safeParse（頂層 z.object 換 looseObject）。
  - 成功：把結果淺合併回 stat_data；失敗整條丟掉。
  - 最後清空指令陣列，MagVarUpdate 套用迴圈就不跑。
  - 收尾時 schema 換成佔位字串，並清掉 display／delta。
- 指令值走 zod 自己的解析（`value-parser.ts` 的 `zod-value`），沒有數學式。
- 兩處刻意偏離上游：
  - 上游用 new Function 求值物件／陣列字面量，這裡改用 JSON5，含運算式的字面量因此走 YAML；理由是模型輸出永不當程式執行〔作者裁決 2026-10-27〕。
  - 上游用 lodash 存取路徑，會走到繼承屬性（`constructor.prototype`、`__proto__`），模型寫的路徑因此能改到 Object.prototype。這裡路徑每一層只走自有屬性，寫入用 defineProperty；`__proto__` 等只當成一般鍵名；中途遇到函式也換成新容器，不往函式裡寫〔作者裁決 2026-10-27〕。這組存取（`source-path.ts`）MagVarUpdate 套用路徑也共用：原本 lodash 取容器會走到繼承屬性，`_.insert('constructor.prototype', …)` 等寫法能改到 Object.prototype，網頁宿主在主頁面執行，影響最大〔作者裁決 2026-10-27：本案修〕。
- 值解析回錯（宿主逾時、值超過上限、Worker 掛掉）一律整次失敗（code `eval-failed`），不當單條拒收：zod 版的解析本身不會失敗。
- 開局：`zodInitialize(variables)`，initvar 先 `cleanUpMetadata` 再過 schema。成功淺合併回去；失敗照原值，並回報錯誤。
- transform 留下的非有限數存成 null（同上游 JSON 存檔）。
- zod 桌照上游不做 `$meta` 檢查。網頁 `initializeSchema` 對 zod 卡要跳過（P5）。

### 1.4 回傳契約（P2 定版，型別見 `host-contract.ts`）
`parseMessageDetailed(message, old)` 回傳：
```
{ table, zod, results: [{index, op, path: string[], to?: string[], status, reason?, issues?, issueCount?}], changes: [{path: string[], before?, after?, truncated?}], changesTruncated }
```
status 四種：
- applied：套用了。
- no-change：過了 schema 但資料沒變，例如巢狀未列鍵被剝掉、刪掉後被 prefault 補回原值。帳本要把它當拒收回饋，免得 GM 以為已寫入。
- rejected：上游會通知 GM 的拒收。reason 為 schema、add-not-number、move-source-missing，或 move-source-<刪除來源那一步的原因>（刪不掉來源時記這一步，不記之後對 null 的 set）。
- ignored：上游靜默丟掉的。reason 為 readonly、no-path（add 沒有路徑）、not-container 或 unsupported。

其他規則：
- 單條拒收：同批其他成功的指令照樣保留。
- 整次失敗（逾時、觸限、例外、結果超出上限）：丟錯，不回部分結果。
- 大小上限：
  - table：沿用 `PARSE_LIMITS`（2 MB 等）。
  - issues：每條最多 5 則、每則 500 字，另附 issueCount。每則是「路径: … ✖ 訊息 → 输入: …」，路徑與訊息在前；超過字數先截輸入那段，尾端加「…」。例外：路徑加訊息本身就超過 500 字時整則照截，路徑也可能被截；完整定位一律以 results 的 path／to 分段欄位為準，錯誤文字只供閱讀。路徑裡鍵名含點或括號的段用 `["…"]` 包起來。
  - results 的 path／to 是分段陣列，鍵名完整保留、不截斷（長度受訊息 256 KB 上限約束），分段和實際寫入一致：set／add／insert 對該條指令開始時的表解析，整串是自有鍵就當單一鍵；delete 與 move 來源照上游一律拆段。
  - changes：最多 2000 筆，超過就設 changesTruncated；單筆值超過 8 KB 只記路徑並標 truncated。
- results 最多 1000 條，與指令數上限相同。
- 「只驗證」API 若在 P4 加上，網頁版同一輪補上。

### 1.5 schema 載入與選擇規則（P2 定版）
- 偵測：tavern_helper 腳本裡 `enabled` 不為 false、內容含 `registerMvuSchema` 的那一支，存原文。停用的腳本不算，照上游不跑，該桌就是非 zod 桌。
- 同一桌有兩張卡都偵測到：用先匯入的那張；後面那張不執行，明白告知玩家〔模型判斷·未裁決，主線裁〕。同一張卡有多支：取腳本順序的第一支。
- 下列情況都算載入失敗：腳本執行拋錯、逾時、觸限、import 白名單外的模組、執行完沒呼叫 `registerMvuSchema`、`registerMvuSchema` 展開 schema 時拋錯（核心照上游在註冊時就展開一次）。載入失敗時，該桌 MVU 更新整次失敗並回饋，不退回 MagVarUpdate〔模型判斷·未裁決，主線裁〕。
- 腳本內重複呼叫 `registerMvuSchema`：只認第一次，其餘只計入 `zodStatus().duplicates`。
- 執行環境只放 `z`、`_`、`$`。QuickJS 的 loader 白名單只把 mvu_zod 的網址對應到墊片模組，墊片轉呼叫核心的 `registerMvuSchema`。

### 1.6 對照測試
- `source-zod.test.ts` 用自製的乾淨 schema（`fixtures/clean-zod-schema.txt`，構件照真卡），跑 41 案（`fixtures/zod-corpus.json`），涵蓋：insert 遇 nil 先建物件再建陣列、move 物件／陣列／頂層、別名、JSONPointer 跳脫、拒收後的後續指令、自訂 transform。非冪等 transform 另有單獨測項。
- 預期值照上游原始碼逐案推導後手寫，再於本機直接執行上游 `mvu_zod.ts`（29becf3）自動比對：41 案的最終表、applied 與 rejected 的指令序號，以及開局結果，全部一致。上游為 PolyForm Noncommercial 授權，對照程式只在本機跑、不入版控〔作者裁決 2026-10-27：放行自動比對〕。
- 三張真卡（TestCards）只在本機跑，CI 略過。

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
  - interrupt 中斷要在呼叫 done 之前拋出，done 之後再拋會被吃掉。不可把中斷包成 `{ok:false}` 回覆：`zod-value` 收到 `{ok:false}` 會整次失敗，但非 zod 的 `value` 會變成單條失敗並繼續跑。
  - interrupt 無法替閒置中的未結算 Promise 產生回覆；宿主不得留下等外部事件才結算的請求。
  - 執行環境要自備 `TextEncoder`（`value-parser.ts` 直接 `new TextEncoder()`）。
  - schema 載入失敗時（§1.5），核心只會回報未註冊；宿主若照樣呼叫 parseMessage，這桌會悄悄退回 MagVarUpdate。所以 P3 宿主遇到載入失敗必須停下：該桌 MVU 更新整次失敗並回饋，不呼叫核心。兩張卡衝突不停桌，照 §1.5 用先匯入的那張、拒絕後面那張並告知玩家，該桌照常更新。
  - zod 迴圈用 try/catch 把 schema／transform 拋的錯當單條拒收；QuickJS 的 interrupt 是不可捕捉的錯誤，應穿過 catch 變成整次失敗。P3 要有測試釘住這點（transform 裡無窮迴圈 → 整次失敗，不是單條拒收）。

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
