# card-shim-no-zod 做法（已結案）

zod 桌卡片前端自己呼叫 `Mvu.parseMessage` 時，結果要跟後端（桌面 QuickJS、網頁隔離 Worker）對同一輸入算出的表相同。
結案 2026-10-27：做法經 Sol、Grok、主線三方共識，驗收兩輪過（第 2 輪 Sol、Grok 都 DONE）。主線裁決兩項：ENDED 系列補舊表參數（驗收第 1 輪採 Sol，Grok 認為不擋）、STARTED 改帶更新前的表〔模型判斷·主線裁決〕。未標來源的細節是施工判斷〔模型判斷·未裁決〕。

## 1. 重現（2026-10-27）
- 方法：同一份 `buildMvuEngineSource()` 在 Node 建「沒有 z 的核心」（＝shim 內嵌的那段，值解析同 Worker 的 `runEval`）當前端，對照 `createDesktopEngine()` 載真卡 schema（＝桌面 QuickJS 入口，Node 與 QuickJS 由 golden 對照過）當後端。三張真卡（`fixtures/real-cards.ts` 那三張）開局後逐條跑 `REAL_CARDS` 的指令。前端那份不是真 iframe 跑的，驗收測試要走真訊息通道（4 節）。
- 結果：17 條有 14 條兩邊的表不同，分兩類，驗收兩類都要收——條件寫成「同一輸入兩邊的結果表相同」，不只看接受／拒收：
  - 後端拒收、前端照寫：`_.set('基础信息.时间', 7)`（後端留「未知」、前端寫 7）；`_.set('基础信息.已覆灭小队数','x')`（0 → null）；`_.insert('便器数据库','T999',…)`（後端拒、前端建）；`_.set('便器数据库.T001.基础档案.评级','普通')`；`_.set('用户数据.积分','abc')`（100 → null）；`_.set('World.Roll20','abc')`（10 → null）。
  - 兩邊都套用但值不同（transform／prefault／剝鍵）：`_.add('World.Invasion', 500)` 後端夾到 100、前端 501；`_.set('Player.MP', 500)` 後端 "500/500"、前端 500；`_.set('Player.HP','12000/10000')` 後端夾成 "10000/10000"；`_.set('…开发阶段', 9)` 後端 5；`_.insert` 新角色後端補齊預設鍵、前端只有給的鍵；`_.set('Player.Inventory', …)` 後端照 schema 濾掉部分項目（數量 0 與超量的雜物）、前端全收。
  - 相同的 3 條：add 到非數字（兩邊都不動）、`'250'`、`_` 路徑。

## 2. 上游怎麼跑
酒館助手的 `initializeGlobal('Mvu', …)` 把 Mvu 掛在酒館主頁的 window，前端 iframe 的 `Mvu` 是指向它的 getter（JS-Slash-Runner `src/function/global.ts`）。所以卡片前端呼叫 `parseMessage` 跑的是 MVU 腳本那一份 `updateVariables`，mvu_zod 的監聽器在同一條事件匯流排上——前端與處理 AI 回覆的是**同一台引擎**。本 app 的 iframe 自帶一份沒有 schema 的核心，才是偏離。

## 3. 做法：zod 桌的 parseMessage 交宿主的 zod 引擎跑〔三方共識〕
- 不把 zod 搬進 iframe：zod 打包 454 KB（gzip 93 KB）要內嵌進每支 iframe（網頁每則訊息一支）；卡片 schema 腳本得在前端 iframe 的 realm 執行（上游也不是這樣）；不透明來源內 blob import 在 WKWebView 沒驗過；兩個執行環境各跑一份，一致只能靠測，不是保證。
- 交宿主跑：與後端一致由「同一台引擎、同一份 schema」保證，也就是上游的結構。

### 3.1 誰判斷是不是 zod 桌：宿主〔三方共識〕
- shim 的 `parseMessage` 先送 `mvu-parse`，宿主回 `zod:false` 就照舊在 iframe 跑核心（非 zod 桌行為不變）、回結果就用結果。代價：非 zod 桌每次也多送一趟整張 old 給宿主；只在卡片自己呼叫 parseMessage 時發生。
- 宿主要分清「確認不是 zod 桌」與「判定失敗」：判定失敗回錯（iframe reject），不可回 `zod:false` 悄悄退回 MagVarUpdate。
  - 桌面：`wiring::zod_table_checked` 回 `DataResult<Option<ZodTable>>`，讀 state、卡清單、重構標記失敗都是 Err；原 `zod_table` 改成它的 `.ok().flatten()`，GM 回合等既有呼叫端行為不變。
  - 網頁：卡上沒有 schema 腳本（`findSchemaScript` 為 null）才回 `zod:false`；有 schema 但執行期還沒載好（`runtime.zod` 為 null）回錯 `not-ready`。

### 3.2 訊息
- `mvu-parse`（iframe → 宿主）：`requestId`、`token`、`message`（字串）、`old`（JSON 字串，保留鍵序）。
- `mvu-parse-result`（宿主 → iframe）：`{ ok: true, zod: false }`｜`{ ok: true, zod: true, table }`（table 是 JSON 字串）｜`{ ok: false, error }`。不進序列佇列，直接結算。
- iframe 送出前先擋：參數型別（同核心 `runParse`）、訊息 256 KB、整表 2 MB（核心 `limits.TABLE_BYTES`，共用核心回傳 `limits` 給宿主比照），超過直接 reject 不送。
- 同一支 iframe 同時只送一筆（後到的排隊）；10 秒沒回 reject（code `timeout`），之後晚到的回覆不認。

### 3.3 失敗語意
判定失敗、還沒載好、schema 載入失敗、逾時、腳本出錯、輸出不合契約 → `parseMessage` reject（code `host-failed`／`timeout`，訊息帶宿主的錯誤文字），不退回 MagVarUpdate 語意。

### 3.4 事件：只補通知〔三方共識〕
- 回覆抵達後依序：先放在途名額 → 補發 STARTED → ENDED → ENDED_for_zod → resolve。名額先放，ENDED 監聽器裡再 await parseMessage 才不會互等到逾時。
- 參數同核心：STARTED 帶更新前的表；ENDED、ENDED_for_zod 帶（最終表, 舊表）。舊表是送宿主前那一份（驗收第 1 輪補：卡片監聽器常讀舊值）。每支監聽器各拿參數的拷貝，改不到回傳值；不補 COMMAND_PARSED 系列、SINGLE_VARIABLE_UPDATED（zod 迴圈本來就不逐條發）。

### 3.5 桌面
- Tauri command `card_vars_parse(world_id, message, old_json)`（`commands/card_vars.rs`，歸讀取類）：巨集同卡寫入與 GM 回合（`{{user}}`＝玩家卡名，沒有就照介面語系的稱呼；`{{char}}` 經 `engine_macros` 退回帶 schema 的卡名），不信 iframe 給的；`spawn_blocking` 裡跑 `wiring::card_parse`。
- `wiring::card_parse`：`zod_table_checked` → `shared().run_batch(...)`，回 `detailed.table` 序列化（serde_json 開了 preserve_order）。只算不寫：不碰 state、不記帳、回饋句丟掉。錯誤文字開頭是失敗種類（`detect:`、`bad-input:`、`SchemaLoad`、`Deadline`…）。
- 卡片沙盒的 MVU 訊息處理抽成 `card-interface/mvu/card-mvu-messages.ts`（寫入的形狀檢查、值解析、parse 服務；網頁共用 `serveParse`）。controller 保留 iframe 身分、token、生命週期判定，處理模組用參數接依賴、不存狀態。

### 3.6 網頁
- `frontend-host.ts` 加 `case "mvu-parse"` → `deps.mvu.parse`；`useTableMvu.parse` 交同一支 zod runner（與 AI 回覆處理同一條 tail，依序派工）。

### 3.7 不動
- `setMvuVariable`／`getMvuVariable`：上游也不過 schema。
- `replaceMvuData`／`card_vars_write`：照上游直接寫、不另立案〔三方共識〕。
- `src-tauri/src/import/`。

## 4. 已知限制
- 事件是通知相容，不支援監聽器介入運算：COMMAND_PARSED 監聽器改指令、ENDED 監聽器改表都影響不到結果；上游 ENDED 當下表裡還有 display_data／delta_data，本案補發的最終表沒有（zod 迴圈收尾已清掉）。
- 卡片可以直接寫入未過 schema 的值（replaceMvuData／replaceVariables 照上游不驗），之後的 parse 再依 zod 處理。
- 逾時固定 10 秒是產品取捨：網頁 zod 引擎排隊後還可能有冷啟動（最多 10 秒開機＋5 秒運算），第一次呼叫可能 reject；桌面排在 GM 回合後面時同理。

## 5. 驗收
- 結果表用保留鍵序的序列化字串比（作者裁決保留寫入順序），不用忽略順序的相等。
- 真卡 17 條（TestCards 本機限定，CI 略過）：`card-mvu-parse.test.ts` 經沙盒通道拿到的表與宿主引擎逐字相同，並寫出 `TestCards/card-parse-golden.json`；cargo（QuickJS `run_batch`）與網頁（`zod-table.test.tsx`）拿同一份逐字比。網頁那邊用的是測試 runner 替身（`zod/test-runner.ts`，同一個 QuickJS 宿主在同進程跑），不是真的隔離 iframe＋Worker。
- 沒驗到的：真的不透明 iframe／WKWebView 裡那段 shim（排隊、補發事件、逾時）只有 Node 單元測試；測試通道只驗了宿主端整條（controller → `card_vars_parse` → QuickJS）。
- shim（`card-mvu-parse.test.ts`）：zod:false 走本地核心；zod 桌用宿主表、事件順序與內容；回錯 reject 不退回；送前上限；單筆在途；ENDED 監聽器 await 下一次 parse；逾時 reject 與晚到結果不認。
- 桌面 controller：token 相符才交 `card_vars_parse`、錯誤回給沙盒、不碰寫入；cargo：只算不寫、非 zod 桌與重構桌回 None、判定失敗與 schema 載入失敗回 Err。
- 網頁：frontend-host 轉交；TableMvu.parse 走 zod runner、整次失敗回錯、還沒載好回 not-ready、非 zod 桌回 zod:false。
- 實機：測試通道開 zod 卡（HeroTrainingUnderSide_stats）的卡片介面，iframe 呼叫 parseMessage 與後端一致。
- `npm run verify` 綠。

## 6. 業界做法與差距
- 業界：驗證只放在一處可信宿主，沙盒裡的 UI 要預覽結果時走 RPC 請宿主 dry-run（伺服器端驗證＋預覽端點），不把驗證器另打包一份進沙盒；上游 ST 也是同一台引擎（Mvu 掛主頁全域）。
- 差距：補上後預覽與正式結果同一台引擎；剩 4 節的事件介入。
