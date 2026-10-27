# mvu-zod-semantics 規格

zod 卡照上游 mvu_zod，用真 zod 執行卡片自帶的 schema；桌面與網頁共用同一份 JS 的 MVU 引擎。拍板見交接檔 [handoffs/mvu-zod-semantics.md](../handoffs/mvu-zod-semantics.md)。
本檔經 Sol、Grok、主線三方審過；未標來源的細節是施工判斷，標〔模型判斷·未裁決〕。

## 0. 定案
- 桌面宿主用 rquickjs `=0.14.0`（內含 quickjs-ng 0.16.2），預設 allocator，不開 `rust-alloc`〔三方共識，依 QuickJS 短驗證〕。短驗證證據：mathjs、json5、yaml、jsonrepair、zod 4.6.7 原樣跑通；三份卡片 schema 的輸出與 Node 逐字相同；記憶體、時間、堆疊上限實測生效；沒有網路與檔案 API；觸限後 runtime 可能壞掉，所以觸限就重建。
- 新依賴：npm `zod`、crate `rquickjs`；網頁另加 `es-module-lexer`。三者都是 MIT。根目錄另加開發依賴 `@types/lodash` 4.17.25（同網頁）。
- 發版前要驗：Windows（MSVC）第一次編 quickjs-ng 的 C 程式碼（rquickjs-sys 有預先產生的 bindings，不需要 libclang）。
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
env.host.fatal?(error) -> boolean                    // 選配：資源類錯誤（記憶體、堆疊）判定，真就照拋、整次失敗
env.emit(name, args, isolate) -> Promise              // MVU 事件
env.events、env.lodash
```
- iframe 與網頁宿主：`evaluate` 送 Worker（單筆 200 ms 逾時），給 timers；保險在核心，逾時呼叫取消函式並以 eval-timeout 整次失敗。只適用非 zod 路徑；zod 路徑在 Worker 內同步跑、只靠整批期限（4.2 節）。
- `fatal`：QuickJS 的記憶體不足、堆疊溢位可被 catch，zod 迴圈會把它當單條拒收；桌面宿主給 `fatal`，zod 驗證與開局的 catch 先問它，真就照拋。判定是 InternalError、堆疊溢位的 RangeError、丟 null（記憶體耗盡到建不出錯誤物件）；已知差異：卡片 transform 自己 throw null／undefined 也算整次失敗（上游是單條拒收）〔模型判斷·三方共識〕。iframe 與網頁不給，行為不變。
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
| `initvar.ts` | initvar 解析與合併（parseStructured、correctlyMerge、initvarBody、openingOverride），桌面入口與網頁共用（6.3 節） |
| `zod-feedback.ts` | zod 逐條結果給 GM 的回饋句與 notes 上限，桌面與網頁共用（6.1 節） |
| `quickjs-entry.ts` | 桌面 bundle 入口：`boot()` 補 TextEncoder、安靜的 console 與卡片用的 z、_、$，回 JSON 字串介面（register、status、parse（另帶 feedback）、initialize、seed） |
| `fixtures/zod-corpus-golden.json` | 入口在 Node 跑 41 案的完整輸出，cargo 拿同一份比 QuickJS；重產 `UPDATE_ZOD_GOLDEN=1` |
| `fixtures/real-cards.ts` | 三張真卡的取材與對照指令（vitest 兩檔共用；真卡基準寫到 `TestCards/zod-cards-golden.json` 給 cargo） |

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
- 同一桌有兩張卡都偵測到：用先匯入的那張；後面那張不執行，明白告知玩家〔模型判斷·未裁決，主線裁〕。桌面的「先匯入」看角色 id（ULID，匯入當下配發，側欄拖曳不動它）；世界層原卡（建桌時匯入）排最前〔模型判斷·未裁決〕。同一張卡有多支：取腳本順序的第一支。
- 下列情況都算載入失敗：腳本執行拋錯、觸限、import 白名單外的模組、執行完沒呼叫 `registerMvuSchema`、`registerMvuSchema` 展開 schema 時拋錯（核心照上游在註冊時就展開一次）。載入失敗時，該桌 MVU 更新整次失敗並回饋，不退回 MagVarUpdate〔模型判斷·未裁決，主線裁〕。
- 載入途中逾時歸期限失敗（Deadline），同樣整次失敗、不呼叫核心；「可重試」指玩家重 Roll 時再試，宿主不自動重試〔模型判斷·三方共識〕。
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
- 對外（P3 定版）：`MvuEngine::new(Limits)`；`deadline()` 起算一次整次期限；`run_batch(桌, schema, 訊息, 舊表, 巨集, 期限)` 回 `BatchOutput`（契約＋重複註冊數）；`initialize(桌, schema, stat_data, 期限)` 回清掉元資料、過完 schema 的表或驗證錯誤。同步 API，P4 在 async 裡用 `spawn_blocking` 包。失敗一律 `EngineFailure { kind, detail }`：InputTooLarge、SchemaLoad、Deadline、Memory、Stack、Script{code}、Busy、Crashed、BadOutput。
- 與 JS 之間傳 JSON 字串；回來後 Rust 再驗結果筆數與表大小（卡片腳本與核心同一個 realm，核心自己的上限可能被卡片改掉）。

### 2.2 執行緒與生命週期
- runtime 不是 `Send`：開一條專用 OS 執行緒（stack 8 MB），收 mpsc 請求，用 oneshot 回覆，外層逾時 6 秒。
- 每桌一個 runtime，最多 2 桌，LRU 淘汰。一桌一條執行緒，runtime 在該執行緒上建立。
- 卡片 import 白名單：StageDog/tavern_resource 的 `dist/util/mvu_zod.js`，jsdelivr 三個鏡像（testingcf、cdn、fastly），可帶 `@版本`；對應到墊片，從不連網〔模型判斷·三方共識〕。
- 以下情況重建 runtime：觸限、例外、schema 雜湊變了、閒置 10 分鐘。
- 上限：記憶體 64 MB、整批 5 秒、堆疊 1 MB。
- 單一 monotonic deadline 涵蓋整次處理：schema 載入、解析、逐條驗證、後處理後的第二次驗證、輸出；第二次驗證不重新起算 5 秒。
- 逾時復原：外層逾時一到，就把執行緒標成作廢、改開新的接手。未回收的作廢執行緒有數量上限，重建也有次數上限，超過就整次失敗並回饋，不可無限作廢再開。
  - 全域名額 2：「執行中（含排隊）＋作廢未結束」的請求總數。送出請求前取得名額，執行緒做完才歸還，卡死的一直握著，所以作廢卡死的執行緒不會超過 2 條（被 LRU 淘汰後才逾時的也算在內）。名額滿了新請求一律 Busy：兩條卡死期間所有桌都收到 Busy，回饋要寫明；平常多桌輪流、兩桌同時都不受影響。10 分鐘內作廢重開最多 3 次（每桌計算）：一張卡連續逾時只擋它自己那一桌〔模型判斷·三方共識〕。作廢時把該執行緒的期限設成現在，JS 下一次輪詢就中斷。
  - 被 LRU 淘汰但還在跑的執行緒，呼叫端仍握著它的追蹤，逾時照樣作廢、計數；作廢與執行緒結束誰先都只計一次。
  - 期限一到就整次失敗：worker 跑完（含 GC）、主執行緒收到回覆、Rust 驗完輸出，回成功前都再對一次期限。
  - 中斷處理器觸發過，就算 JS 照常回了結果也不認（整次失敗）。
  - 載入 schema 途中逾時歸 Deadline、其餘載入問題歸 SchemaLoad（1.5 節）。
- panic 用 `catch_unwind` 包住，轉成整次失敗。
- 輸入在進引擎前先照 `PARSE_LIMITS` 擋大小。
- QuickJS 宿主不給 timers，`evaluate` 在同一個 context 直接呼叫 `runEval`，每次都必須回覆（呼叫 done）或拋錯：
  - interrupt 中斷要在呼叫 done 之前拋出，done 之後再拋會被吃掉。不可把中斷包成 `{ok:false}` 回覆：`zod-value` 收到 `{ok:false}` 會整次失敗，但非 zod 的 `value` 會變成單條失敗並繼續跑。
  - interrupt 無法替閒置中的未結算 Promise 產生回覆；宿主不得留下等外部事件才結算的請求。
  - 執行環境要自備 `TextEncoder`（`value-parser.ts` 直接 `new TextEncoder()`）。
  - schema 載入失敗時（§1.5），核心只會回報未註冊；宿主若照樣呼叫 parseMessage，這桌會悄悄退回 MagVarUpdate。所以 P3 宿主遇到載入失敗必須停下：該桌 MVU 更新整次失敗並回饋，不呼叫核心。兩張卡衝突不停桌，照 §1.5 用先匯入的那張、拒絕後面那張並告知玩家，該桌照常更新。
  - zod 迴圈用 try/catch 把 schema／transform 拋的錯當單條拒收；QuickJS 的 interrupt 是不可捕捉的錯誤，應穿過 catch 變成整次失敗。P3 要有測試釘住這點（transform 裡無窮迴圈 → 整次失敗，不是單條拒收）。

### 2.3 bundle
- npm script `build:mvu-bundle`（`scripts/build-mvu-bundle.mjs`，用 vite 的建置 API 打 IIFE、全域名 TTMvu）產生 `src-tauri/gen/mvu-engine.js`（gitignore）。
- 併進 `npm run build`，所以 verify 的 cargo 步驟前、tauri 的 beforeBuildCommand、windows-install-smoke 都會先產生；beforeDevCommand 另外先跑它。
- `build.rs` 檢查檔案存在；缺了就停下，提示要先跑哪個指令。

### 2.4 提交契約
現況鏈：`TurnGuard::commit`（chat.rs:772）→ `apply_gm_block`（write.rs:451）→ `gm_commit_apply`（chat.rs:390）→ `apply_block_typed`（ledger.rs:247）。

zod 桌改成三段：
1. **固定輸入（短鎖）**：`message_vars::turn_input` 沿用 `apply_gm_block` 開頭那組核對。`ensure_active` 物化出的 input 先存回回合紀錄，再回傳 `input_rev`。schema 雜湊與 macros 一起固定；若不固定，就列入失效條件。
2. **鎖外跑引擎**：`run_batch` 回 1.4 的契約。
3. **短鎖重驗才寫**：核對 turn_id、phase、scene、generation、input_rev、schema 雜湊，不符就回 None。P4 實作：段 1 存的輸入整張比對；schema 雜湊在段 3 同一把鎖內重讀這桌生效的 schema 比對。開場白的結果帶三份憑證（初始化來源、桌世代、schema 雜湊），鎖內任一不符算整次失敗、正文照落。
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
- `block.fields` 與清單整支替換：引擎路徑不跑〔模型判斷·三方共識〕。上游 MVU 只認指令，本 app 的狀態區塊欄位與清單不是上游語意；回覆裡有就整批不套，記一筆 Rejected（同介面接管桌的「狀態區塊未套用，請以 UpdateVariable 回報變動」），讓模型下一輪改用指令。引擎收的是整段原始回覆（同上游 parseMessage 掃整則訊息），不是只有 `<UpdateVariable>` 內文〔模型判斷·三方共識〕。
- 帳本：新檔 `mechanism/engine_records.rs`，把逐條結果映射成 `Record`，回饋句沿用 `build_notes`；句子本身與 `_` 路徑的回饋見 6.1 節。
- 開場白走同一條路（transcript.rs:389 → `opening_table`）。整次失敗時照 `state_error` 留正文、不裝變數，不走「世代不符回 None」。
- initvar 的 safeParse 在建 `init_source` 時做一次；`scene_seed_for_next` 不重跑。
- 收回讀已存快照，不重跑 transform。
- 整次失敗：狀態整批不提交，正文照常落檔，回饋「變數更新失敗，可能需要重 Roll」。
- 玩家回饋〔模型判斷·三方共識〕：GM 回合走既有 `state_error`（前端照舊顯示錯誤提示），開場白記一筆 Error 進機制帳本；建種子失敗一併照這套。文字用新的 UiMsg，十語系：
  - `mvu_engine_busy`：「變數引擎忙碌中：全機同時最多處理兩個變數更新（含卡住待回收的），這一輪變數沒有更新，可稍後重 Roll。」Busy 與作廢上限、每桌重開上限都歸這句。
  - `mvu_update_failed_<種類>`：「變數更新失敗，可能需要重 Roll（原因）」，原因各語系一句白話，不塞英文代碼。種類：timeout（逾時）、memory（記憶體）、stack（堆疊）、too_large（回覆或變數超出長度上限）、too_many_commands、schema_load、initvar（initvar 讀不懂）、engine（其餘）。
  - 單條拒收不算失敗：照帳本 Rejected 回饋模型（`engine_records.rs`），玩家端不跳錯。

### 2.4a 物件鍵順序
- 保留寫入順序〔作者裁決 2026-10-27〕：上游在 JS 保留插入順序，依順序的 transform（HeroTraining 物品只留最後 10 個、背包 takeRight）要和上游一致。
- P3 已做：serde_json 開 `preserve_order`（全 crate），引擎進出與存讀往返都保序（cargo 測試 `key_insertion_order_survives_store_and_read_round_trips`）；Node 對照改成逐項完全相同。`Map` 的相等比較本來就不看順序；`ui_msg` 的參數文字、`scene_budget` 的設定世代指紋都照名稱排序，維持原行為（不動存檔順序）。
- P4 接線：
  - 存讀〔模型判斷·未裁決〕：逐字稿事件本來就以保序的 `message_vars::Json` 原文存整張表；zod 桌的新表直接取引擎回的表（serde Map → 文字 → `Json`，全程保序），不經 `StateNode` 樹再物化。`state.json` 的樹只是投影快取，排序無妨。
  - 種子（照上游 initCheck、與 P5 對齊）：zod 桌第一次啟用變數模式時由引擎建：卡上各條 `[initvar]` 原文（收集順序同網頁 `bookEntries`：陣列形照 id 整數由小到大、id 重複後蓋前，物件形照 JS 鍵序，標題只看 comment，內文先剝 `@@`，停用的照收；兩邊跑同一份案例 `src/shared/contracts/mvu-initvar-order.json`）依序解析（YAML／JSON5，保序）合併（陣列整支取代）→ 開場白自帶的 `<initvar>` 覆寫 → cleanUpMetadata → 過 schema，就是「建 init_source 時做一次 safeParse」；開場白本身的指令接著走一般 zod 迴圈。卡上沒有 `[initvar]` 時以狀態樹物化的表為底，同樣走「開場白覆寫 → 過 schema」。建種子失敗（schema 載入失敗、觸限等）＝整次失敗：正文照落、不帶表、這一幕不啟用變數模式（下一輪再建），不改從樹物化。
  - 已知限制〔模型判斷·三方共識〕：卡上沒有 initvar 時種子鍵序依鍵名排序（樹的順序）；種子建好前在面板手改的值不進種子（有 initvar 時）。
  - initvar 解析與合併的小函式在共用核心 `src/features/mvu-engine/initvar.ts`，桌面與網頁共用（6.3 節）。
  - 非 zod 桌的開場白 `<initvar>` 覆寫不在本案，主線另外立案〔模型判斷·三方共識〕。
  - 後處理改到樹時，用 `merge_tree_change` 合回引擎表：只換變了的葉、其餘沿用原 JSON 與原順序。
  - 刪鍵：接線程式碼不對 serde `Map` 刪鍵；若要刪一律 `shift_remove`。

### 2.5 判定
- zod 桌＝未重構桌（Upstream），且有卡註冊 schema。
- `CardInterface.zod` 存腳本原文與 `[initvar]` 原文，偵測寫在 `import/card_schema.rs`。
- 重構桌照舊。非 zod 的 MVU 桌本案照舊走 Rust；等 mvu-command-syntax／mvu-meta-schema 接上引擎後，再清掉只服務 Upstream 的 Rust 碼。

### 2.6 文件例外
`evaluator.rs` 開頭、`import/ejs.rs`、card-mvu-shim 計畫裡「不執行卡片腳本」的紅線，加上例外：只限卡片變數 schema、只在隔離環境執行。P4 已同步改。

## 3. 分包與驗收
| 包 | 內容 | 驗收要點 |
|---|---|---|
| P1 | 共用核心抽出＋host 介面，行為不變 | verify 綠；三宿主的行為與事件順序要逐項比對，不能只拿「測試數量沒變」代替 |
| P2 | zod 迴圈、schema 載入、回傳契約與 schema 選擇規則定版 | 見下方 P2 對照清單 |
| P3 | Rust QuickJS 宿主 | 短驗證全部轉成 cargo 測試；deadline、逾時復原上限 |
| P4 | 桌面接線 | 過期結果不提交；失敗時正文仍落檔；開場白同路；收回不重跑 transform；後處理退回連帶撤回 |
| P5 | 網頁宿主（P2 契約凍結後，與 P3／P4 並行） | 見下方 P5 驗收清單 |
| P6 | 裁決落地（6 節） | parity json 加 zod 分支、zod 回饋句（兩宿主同一份句子）、`_` 路徑回饋、initvar 小函式搬進共用核心 |

P2 對照清單：上游獨立對照，逐項比對：
- insert 遇 nil 時，建物件或陣列；
- move 陣列；
- 指令別名；
- JSONPointer；
- 拒收之後的後續指令；
- 非冪等 transform。

Node 與 QuickJS 結果相同，只證明兩個宿主一致，不能當成和上游一致。

P5 驗收清單（`npm run e2e:zod`，WebKit＋本機 Chrome，P5 必跑、不進 verify）：
- `/mvu-zod` 的 CSP hash 對得上實際送出的開機碼，開發伺服器的 `/mvu-zod`、`/mvu-zod.html` 同；
- 瀏覽器裡跑 41 案與桌面基準逐項相同（jitless），TestCards 在時三張真卡也比；
- 第一道防線單獨驗（不剝全域）：存檔讀寫刪、網路、eval、非 blob 模組、二層 blob 偷載都失敗；
- 整條路：卡片腳本裡巢狀 Worker、存檔、二層 Blob、WebRTC、網路、假結果都失敗；
- 白名單外的 import、非字面量的動態 import 整次失敗，白名單網址的字面量動態 import 照樣載入；無窮迴圈 5 秒作廢、下次重建可用；
- 外站直接開頁或嵌入後送開機訊息，惡意引擎不跑；判定看另開的探測伺服器、卡片回報的例外與存檔哨兵，不靠攔截請求。

順序：P1 → P2 → P3 → P4，P6 最後。

## 4. 網頁宿主（P5）
定案經 Sol、Grok 兩輪審查與兩次小驗證（WebKit＋Chrome）〔三方共識〕。只有 zod 桌（卡上有啟用、含 `registerMvuSchema` 的腳本，取第一支；一桌一卡，兩卡衝突不適用）走這條，非 zod 桌照舊在主頁跑核心＋值解析 Worker。

### 4.1 隔離：不透明來源為主、剝除為輔
- 第一道：`web/public/mvu-zod.html` 掛 `sandbox="allow-scripts"` 的 iframe，來源不透明，瀏覽器不給本站 IndexedDB。標頭 CSP（`site-headers.ts` 建置時算、開發伺服器中介層同一份）：`default-src 'none'; script-src 'sha256-<開機碼>' blob:; worker-src blob:; connect-src 'none'; frame-ancestors 'self'; base-uri 'none'; form-action 'none'; sandbox allow-scripts`。blob Worker 繼承這份 CSP（實測兩家都是）。
- 開機碼只收一次開機：不在框裡（頂層）就不裝監聽；`event.source === parent`、`event.origin` 是本站、要帶兩個 port；之後只走 port。
- 引擎是 classic IIFE 文字（`web/mvu-zod-engine-plugin.ts` 打包成 `virtual:mvu-zod-engine`，整支自足）。主頁傳文字進去建 blob Worker，不需要 CORS，CSP 也不必寫站點網址。Chrome 在不透明來源起不了 module 型 blob Worker，所以用 classic 再 `import()` 卡片模組。
- 第二道：Worker 開機先在閉包抓住 `postMessage`、`URL.createObjectURL`、`Blob`、`JSON`，沿原型鏈把全域剝到白名單（ECMAScript 內建＋TextEncoder／TextDecoder、計時器、structuredClone；`zod/strip.ts`）。失敗即停：indexedDB、caches、fetch、XHR、WebSocket、WebTransport、EventSource、Worker、SharedWorker、RTCPeerConnection、RTCDataChannel、BroadcastChannel、URL、Blob、postMessage、importScripts、navigator 任一還看得到就不載卡片。卡片拿不到 blob 網址，所以二層 Blob 模組建不出來；就算建得出來，CSP 也只准 blob、碰不到同站與外站。
- 卡片 import：主頁用 es-module-lexer（asm.js 版，宿主 CSP 不准 wasm）切開。只准規格 2.2 的 mvu_zod 網址（同 `is_mvu_zod_url`），在 Worker 裡接成墊片 blob。動態 import 以兩邊一致為準〔模型判斷·主線裁決〕：桌面 QuickJS loader 在執行時比對網址、放行白名單（實測字面量、非字面量、模板字串都放行），網頁只能靜態判定，所以放行白名單網址的字串字面量動態 import，非字面量（含模板字串）拒收；兩邊共同成立的是「白名單字面量載入、白名單外拒收」，非字面量指向白名單網址只有桌面放行（已知差異，桌面不改）。其他 import、source／defer phase 一律載入失敗。卡片全域只放 z、_、`$`、安靜的 console；zod `jitless`。
- blob 網址：引擎的在 Worker 起來後收回、卡片的在 import 完收回；墊片的留到整支 Worker 回收（非頂層的動態 import 可能晚點才載墊片）。卡片模組跑完後最多等 100 輪事件迴圈讓非頂層 import 完成註冊，仍沒註冊才算載入失敗。

### 4.2 期限與生命週期
- 一桌一支，暖的跨訊息重用；同一桌依序派工、不重疊。
- 值解析在 Worker 內同步跑、不給 timers（同 QuickJS），只靠整批期限；1.1 節「單筆 200 ms、給 timers」只描述非 zod 路徑。
- 開機（取引擎文字＋iframe＋剝除與檢查到 ready）另給 10 秒，卡片腳本這時還沒執行。
- 派工起算單一 monotonic 5 秒：import 卡片與展開 schema、運算、主頁驗回傳、回成功前最後一次對期限。逾時算 Deadline，丟棄那支之後的所有回傳（關 port、叫 iframe terminate、移除 iframe）。載入途中逾時也算 Deadline，不呼叫核心。
- 出錯或逾時就回收，等玩家下一次操作再重建，不自動重試。Chrome 對忙碌中的 Worker 下 terminate，約 2 秒後才真停（WebKit 約 0.3 秒），回傳已丟棄，只多耗 CPU。
- 沒有記憶體硬上限（0 節，另案 web-mvu-memory-cap）。

### 4.3 回傳與提交
- 宿主端用 `parseMessageDetailed`，表以 JSON 文字往返（同桌面語意）。主頁重驗形狀、results ≤ 1000、changes ≤ 2000、表 ≤ 2 MB（`zod/validate.ts`）。逐條拒收回饋給 GM 見 6.1 節。
- 開局與桌面同序：世界書 initvar → 開場白 `<initvar>` 覆寫 → cleanUpMetadata → zodInitialize（只一次；續玩、收回不重跑）→ 開場白指令走 zod 迴圈。zod 桌不生成 MVU schema，schema 欄放上游佔位字串。
- 提示分開：
  - 初始值讀不懂：沿用舊橫幅。
  - zodInitialize 沒過：照原值開局，另一句提示。
  - 整次失敗（載入、逾時、出錯）都是可清除的聊天錯誤、不說整桌沒有變數，下一次成功就清掉，依時機各一句：開局失敗（還沒有開局變數，下一次送出前、組提示之前重試；同一輪回覆後不再試）、開場白指令沒套用（變數停在初始值）、回覆更新失敗（這則不寫變數、停在上一則）。
- 開發模式：宿主頁本身沒有 CSP，隔離宿主頁照樣帶同一份標頭。
- 上線後抽查一次 Cloudflare Pages 上 `/mvu-zod`、`/mvu-zod.html`（308）的實際回應標頭（本機託管伺服器的 308 不帶標頭，驗不到）；本包不擋。

### 4.4 已知缺口
- 卡片 iframe 的 shim 核心沒有 z（1.1 節）：卡片前端自己呼叫 parseMessage 時，zod 桌照樣走 MagVarUpdate 語意，可能接受 zod 路徑會拒收的值（主線另立案）。

## 6. 裁決落地（P6）
上游 mvu_zod 對逐條錯誤只跳玩家通知（toastr），沒有回饋 GM 的機制；下面這套是本 app 的設計（同 mechanism-error-feedback：GM 改得了的錯誤就回饋）〔模型判斷·三方共識〕。

### 6.1 回饋句（GM 看到什麼）
- 句子只有一份：共用核心的純 TS 模組 `src/features/mvu-engine/zod-feedback.ts`，吃 1.4 節的 `results` 逐條出句；桌面經 QuickJS bundle 拿文字（`quickjs-entry.ts` 的 parse 回傳另帶 `feedback`，與 results 一一對應，P2 凍結的 `parseMessageDetailed` 契約不動），`engine_records.rs` 只把文字映射成 Rejected 記錄、不再自己組句；網頁在主頁直接呼叫。
- 路徑寫法同 `_.set` 的路徑（核心 `formatPath`：鍵名含點、括號、引號的段寫成 `["…"]`），空路徑寫「整張表」。op 照引擎的指令型別（JSONPatch 的 replace 會寫成 set）。
- 各狀態的句子：
  - rejected／schema：「{路徑} 的 {op} 被卡片 schema 拒收、維持原值：{錯誤；錯誤}（共 N 則）」，總數多於列出的才補括號。
  - rejected／add-not-number：「{路徑} 的 add 沒有套用、維持原值：{錯誤}」（錯誤帶原值與型別，照上游文字）。
  - rejected／move-source-missing：「move 的來源 {路徑} 不存在，沒有搬。」；move-source-<原因>：「move 的來源 {路徑} 刪不掉、沒有搬：{錯誤}」。
  - no-change：「{路徑} 的 {op} 通過了卡片 schema，但值沒有變（schema 沒列的鍵會被剝掉、被刪的會補回預設值），沒有寫入。」
  - ignored／readonly（`_` 路徑）：「{路徑} 裡的「{第一個 _ 開頭的段}」以 _ 開頭，是唯讀欄位，這條 {op} 沒有套用。」；move 擋在目的地時指出目的地：「move 的目的地 {目的地} 裡的「{段}」以 _ 開頭，是唯讀欄位，沒有搬。」；來源是唯讀（rejected／move-source-readonly）：「move 的來源 {路徑} 裡的「{段}」以 _ 開頭，是唯讀欄位，沒有搬。」
  - 其餘 ignored（no-path、not-container、unsupported）不回饋，同上游靜默；列已知限制，之後有需要再做〔模型判斷·三方共識〕。
  - op 照引擎的指令型別：JSONPatch 的 replace／delta 在句子裡寫成 set／add〔模型判斷·三方共識〕。
- 上限同桌面 `build_notes`：依結果順序、同文去重、最多 5 條、每條最多 240 個 Unicode 碼點（超過截到 239 補「…」）。兩邊跑同一份契約 `src/shared/contracts/mvu-zod-feedback.json`（results → 預期句子與 notes），cargo 與 vitest 都跑。
- 一致的範圍是「引擎逐條結果 → 句子」：桌面的 notes 還會混進狀態區塊未套用（2.4 節）與本地後處理（重擲、觸發、衍生）的拒收，網頁沒有這兩種〔模型判斷·三方共識〕。
- GM 看到的位置：
  - 桌面：照舊在狀態區段「## 上一輪被系統擋下的更新（請照這些現值修正）」（英文提示用英文標題），notes 存 state.json，收回時跟著快照回去。
  - 網頁：notes 掛在那則 AI 回覆的 ChatEntry 上；組提示時取這批逐字稿裡最後一則 AI 回覆的 notes，做成一則 system（標題跟著介面語系，十語系；每條一行），交給 `composePrompt` 當固定段落：先佔預算、接在最後（post_history_instructions 之後），佔不下整次不送，選模判斷同一份結果〔模型判斷·三方共識〕。重 Roll 時被換掉那則不在送出的逐字稿裡，回饋跟著消失。
  - 已知限制：網頁存檔格式（共用契約 web-save）不存 notes，重新載入後上一輪的回饋不保留；之後有需要再做〔模型判斷·三方共識〕。
  - 桌面英文提示用英文標題、其餘用中文標題；網頁十語系各一句。句子本身兩邊都是中文。
- 玩家端照舊只看整次失敗的提示（2.4、4.3 節），單條拒收不跳錯。

### 6.2 parity json 的 zod 分支
- `src/shared/contracts/mvu-replace-parity.json` 每案加 `zod`：同一份 patch 走 zod 迴圈，schema 用全放行的 `z.object({})`（核心與上游都把頂層換成 looseObject，所以任何值都過），隔離「指令語意差異」：預期 stat_data＋逐條 status。差異的典型：數字字串照字串寫、轉不成數字不變 null、值帶說明（`[值, 說明]`）整個換掉不只改第 0 項、路徑不存在照建。
- 另加 `zod_cases`：一組帶真 schema 的小案例——coerce 轉數字、夾值、轉不成數字整條拒收保留舊值、prefault 補回→no-change、嚴格物件剝鍵→no-change、`_` 路徑→ignored、add 到字串→rejected、頂層刪鍵與 move。
- 預期值一律用本機上游對照（`scratchpad/zod-oracle`，跑上游 `mvu_zod.ts`，不入版控）逐案比出來：stat_data、applied 與上游會回報的指令序號都要相同；不只拿我們的核心產生。
- 跑的地方：vitest（Node 核心）與 cargo（QuickJS，經 `run_batch`）讀同一份檔；網頁 Worker 跑的是同一份核心，e2e:zod 照跑。

### 6.3 initvar 小函式搬家
- `parseStructured`、`correctlyMerge`、`initvarBody`、`openingOverride` 搬到 `src/features/mvu-engine/initvar.ts`，桌面 `quickjs-entry.ts` 與網頁 `web/src/features/mvu/initvar.ts` 都改 import 這份，舊副本刪掉；網頁的 `initvarEntries`、`initialTable`（MagVarUpdate 開局路）留在 web。`mvu-initvar-order.json` 兩邊照舊要過。

## 5. 結構
- 已超過 1000 行的檔：parse-source 1047 行，P1 拆；`commands/chat.rs` 1486 行與 `data/scene/transcript.rs` 1248 行，本案只加呼叫，拆檔另立案。
- 接近上限的檔，新責任放新檔：`ledger.rs` 876 行 → `engine_records.rs`；`import/interface.rs` 933 行 → `card_schema.rs`。
