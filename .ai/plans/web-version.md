# web-version 計畫：網頁版引流入口

交接檔：[handoffs/web-version.md](../handoffs/web-version.md)。查證日期一律 2026-10-07，行號以合 main 後的 `web-version` 分支（main 12bfb30）為準。

## 一、範圍

### 1.1 已裁決（照抄）
- 網頁版是免費引流入口、玩完引導下載桌面版；桌面版是主力。〔作者裁決 2026-10-07〕
- 網頁版的桌檔與卡片（角色卡＋內嵌世界書的複合卡）要能跟桌面版通用，見「二之二、格式契約」。〔作者裁決 2026-10-07〕本計畫把「通用」落成：卡片兩端可用、網頁桌檔可匯入桌面；不做桌面桌檔回網頁（D15）。
- 導流與待議各項（D1–D29）的拍板見第五節。〔作者裁決 2026-10-07〕
- 與桌面版分開做、只保留基礎功能、不常更新。〔作者裁決 2026-09-30〕
- 功能架構與頁面美感全新設計；只沿用使用者看不到的底層功能。〔作者裁決 2026-09-30〕
- 重構按鈕、快取命中／用量頁不進網頁版。〔作者裁決 2026-09-30〕
- 卡片內嵌世界書只做關鍵字觸發，不做編輯器。〔作者裁決 2026-09-30〕
- 網頁存檔能匯入桌面版，桌面版新增對應匯入。〔作者裁決 2026-09-30〕
- 支援卡片介面（原卡直玩的介面渲染與 MVU 變數），盡量跟 ST 一樣。〔作者裁決 2026-10-04〕
- 有「去哪裡找卡」導流；爬別站／另存別站卡的站不收。〔作者裁決 2026-09-30〕

### 1.2 專案硬規則（記憶，照辦）
- 單張角色卡是主流：網頁版只做一桌一張卡，多角色列進下載頁功能對照。
- 外部資料只走官方 API、用玩家授權金鑰打官方路徑；不爬網頁、不讀玩家網頁資料。
- 不替玩家花錢：會動到玩家額度的事不自動代做（試打不做；換模重送見 3 節包 1）。
- 不把玩家送去 CLI。
- 沒重構的卡照 SillyTavern 的行為做，不藏功能省工（世界書觸發、巨集、提示組裝、regex 腳本都照 ST）。

### 1.3 本輪確認的範圍〔模型判斷·未裁決〕
- 進：OpenRouter 一鍵授權（PKCE）＋貼金鑰備援、穩定免費自動選模與失效換模、今日免費次數、內建範例卡、匯入單張卡（PNG／JSON，V1／V2／V3）＋選開場白、串流／停止／重新生成／編輯刪除最後一則、卡內世界書觸發、卡片介面（regex 腳本＋沙盒 iframe＋MVU 讀寫）、瀏覽器存檔＋匯出／匯入網頁存檔、下載頁（版本與連結執行時抓）、找卡導流、十語系。
- 不進、改在下載頁功能對照表列出：CLI 訂閱、一句話開桌、狀態欄／機制、多角色、GM、編輯器、生圖、畫廊、贊助、重構、用量頁。

## 二、架構

### 2.1 目錄與建置
- 同 repo 的 `web/`，自己一份 `package.json`／lockfile／`tsconfig.json`／`vite.config.ts`／`vitest.config.ts`；Vite＋React＋TypeScript（與桌面版同棧，才能沿用純 TS 底層）。產物 `web/dist/` 是純靜態檔。
- `web/src/` 結構照 docs/STRUCTURE.md 同一套規則（`features/<name>/`、禁垃圾桶目錄、測試同住）；`web/src` 根層入口另列。
- 桌面版純 TS 底層用路徑別名直接 import，不複製（D2）。

### 2.2 要跟著改的 repo 設定（查證見 4.4）
- `vitest.config.ts`（根）exclude 加 `web/**`，否則根 `npm test` 會收進 web 的測試。
- `scripts/check-structure.mjs` 目前只走 `src/`（第 12 行 `SRC`）：改成同一套規則也走 `web/src/`，根層允許清單分開列。
- `scripts/verify.mjs` 加 web 步驟（web 的 vitest＋build）；`.github/workflows/verify.yml` 多一步 `npm ci --prefix web`。
- `docs/STRUCTURE.md` 補一節「web/」。
- 部署 workflow 在包 9 才建，手動觸發（D12）。

### 2.3 資料流
- 瀏覽器直連 `openrouter.ai/api/v1`（CORS `*`，見 4.1）；沒有自家後端、沒有代理。
- 金鑰只留在瀏覽器，存 localStorage、附登出鈕（D7）；不進網址參數、匯出存檔、錯誤訊息、日誌、postMessage，也不進卡片 iframe。
- 桌面版版本與下載連結：執行時打 `api.github.com/repos/TaoGongSun/Table-Tavern/releases/latest`（見 4.5）。
- 找卡：五站都只放連結，不打站方 API（D6）。
- 存檔：IndexedDB，啟動時請求 `navigator.storage.persist()`；Safari 可能清資料要在存檔處講清楚並提供匯出。事實：WebKit ITP 對 7 天沒互動的網站刪掉 IndexedDB、localStorage 等腳本可寫儲存，加到主畫面（iOS）／Dock（macOS）的網頁 App 豁免（https://webkit.org/tracking-prevention/）——存檔與金鑰（D7）都受影響。

### 2.4 安全
- **PKCE**：`code_challenge_method=S256`；每次授權產一次性隨機 `state`，與 verifier 存同一分頁（sessionStorage），回呼驗過即消耗。回呼頁順序寫死：回呼參數（含錯誤、取消）讀進記憶體→立刻 `history.replaceState` 清網址→驗 state→交換一次；清理前不載任何第三方資源；全站 `<meta name="referrer" content="no-referrer">`。理由：授權碼與 state 不能經 Referer、歷史紀錄或重跑外洩。
- **卡片 iframe 橋接**：照桌面版 `sandbox="allow-scripts"`（不給 `allow-same-origin`），iframe 是不透明來源，讀不到主頁儲存。宿主收 postMessage 一律先核 `event.source === 目前 iframe 的 contentWindow`，再核文件 token、桌世代、資料形狀，不符就丟。驗收含舊 iframe 殘留訊息與外部視窗偽造訊息。理由：不透明來源的 `event.origin` 是 `"null"`，只能靠來源視窗與 token 認人。
- **宿主頁不外連**：宿主頁只連 openrouter.ai、api.github.com；卡片文字、顯示 regex 結果與模型輸出裡的外部圖片在渲染時換成替代文字、不留網址（`web/src/shared/ui/host-markdown.ts`，只載 data: 與同源），CSP `img-src 'self' data:` 是第二層。理由：顯示 regex 能把 `{{lastUserMessage}}` 塞進圖片網址，一載圖就外送對話。卡片 iframe（包 6）內的外連另有提示（下一條）。
- **卡片腳本外連**：卡片腳本可以對外 fetch，等於能把它讀得到的對話內容送出去；金鑰拿不到，但對話內容會外流。寫進風險欄，並在第一次打開卡片介面時提示玩家。
- **CSP 分兩份**：宿主頁不載第三方 JS（只連 openrouter.ai、api.github.com）；卡片另用較寬的政策（卡片常載外部字型與圖片）。srcdoc 文件會繼承宿主 CSP，額外政策只能收緊不能放寬（CSP3〈Initialize a Document's CSP list〉的繼承規則），blob:／data:／about:blank 同樣繼承，所以「宿主嚴、卡片寬」不能靠 srcDoc 做。做法：卡片 iframe 載入 `web/` 內一份獨立靜態沙盒文件（例如 `sandbox.html`），它自帶寬鬆 CSP（meta 或 `_headers`），iframe 仍掛 `sandbox="allow-scripts"`、不給 `allow-same-origin`（維持不透明來源）；卡片內容與沙盒內建庫由宿主 postMessage 送進去，沙盒文件再寫入自己的文件。宿主若用站點級 CSP 標頭，規則必須排除 `sandbox.html` 這個路徑：被站點標頭罩住時，它自己的 meta 只能再收緊，寬政策不會生效；卡片那份寬政策由 `sandbox.html` 自己的回應標頭帶。託管定為 Cloudflare Pages（D1），用 `_headers` 按路徑設：宿主 CSP 標頭排除 `sandbox.html`，卡片寬政策由 `sandbox.html` 那條路徑的標頭單獨帶。
- **顯示安全**：卡片 metadata（名稱、描述、作者註記）與模型輸出在宿主頁一律走安全渲染（DOMPurify，`src/shared/ui/story-markdown.ts:73`），不直接塞 HTML。
- **測試斷言**：匯出存檔的測試要斷言沒有金鑰欄位、沒有 `sk-or-` 字串。

## 二之二、格式契約（網頁版與桌面版通用）

作者最在意的是桌檔與卡片兩邊通用〔作者裁決 2026-10-07〕。契約範圍：卡片兩端可用、網頁桌檔可匯入桌面（單向，D15）。契約本文：卡片 `src/shared/contracts/card-view/card-view.md`、桌檔 `src/shared/contracts/web-save/web-save.md`（含 fixture），改契約＝兩邊同一筆 commit 一起改。

### 卡片契約
- 兩邊同一套解析規則：
  - PNG 只讀 tEXt `chara`（V2）優先、沒有才 `ccv3`，與桌面版現況一致（`src-tauri/src/import/card_io.rs:34-60` 只認 `tEXt`，不收 iTXt／zTXt）；要改收 iTXt／zTXt 就兩邊一起改，並進黃金檔。
  - JSON 收 V1（平鋪）／V2／V3（`data` 包一層），保留完整外殼（`spec`、`spec_version`、未知欄位），不只 `data`。
  - `character_book` 條目收陣列形（V2）與物件形（ST 獨立世界書）。物件形一律算成有條目、條目照 uid 鍵展開。桌面版世界書路的 `has_entries`（`import/card.rs:474`）現在只認陣列，物件形會掉進人設欄轉換——這是要修的錯，不是契約；包 2 一併把它改成同一規則。
  - 介面資產＝`extensions.regex_scripts` 的腳本（顯示用與送模前分流，見包 2）與 MVU 載入判定（`src-tauri/src/import/interface.rs:129-260`）。
- 世界書那條路不得漏（專案記憶：世界書路徑要跟角色卡對等）。桌面版有兩條匯入路：角色卡路（`import/card.rs:257`，把 `character_book` 併進桌的世界書）與世界書路（`import/files.rs:58`→`card.rs:458`：剝 `character_book`、條目空時把人設欄轉常駐條目、存原卡介面檔、卡擴充欄位）；分路由 `probe_import`（`card.rs:136`）判斷。網頁版照同一分路規則，世界書卡（例：`TestCards/main_*_spec_v2.png`）不能只走角色卡那條。
- 機械測試（驗收項）：定一份「卡片正規化檢視」JSON，至少含名稱、各欄位、`extensions`、開場白清單、正規化後的世界書條目（含條目識別）、物件形 entries 的處理結果（預期值＝有條目、照 uid 鍵展開；不得把桌面版現行掉進人設轉換的結果收成黃金檔）、介面腳本清單、是否載 MVU、分路結果。Rust 端加測試把卡算成這份檢視，TS 端同樣算，兩邊對同一份黃金檔比對。fixture 兩層：進 repo 的小型自製複合卡（角色＋內嵌世界書＋regex 介面＋MVU，比照 `scripts/harness-fixtures/mvu-write-probe.json`，含物件形 entries 與 iTXt 對照各一）讓 CI 跑；`TestCards/` 實卡（WestFantsy、NorthHall、DongeonMaster、bcd368…、`main_*_spec_v2.png` 世界書卡）本機跑。

### 桌檔契約（網頁存檔）
- 小而穩、帶版號：頂層 `format`＋`version`（整數）；讀到不認得的版號就拒收並說明，不猜。
- 內容四類：
  1. **卡的指認**：完整原卡 JSON 外殼原封不動（含 `spec_version` 與未知欄位）＋可選原 PNG；兩者不一致時以 JSON 為權威。另存實際採用的匯入身分（`probe_import` 的分路結果），匯入端照存的身分走、不重新推測——避免兩邊推測規則日後分歧時同一份存檔換了路。另存玩家是否允許卡內 regex 腳本（D21）；桌面版怎麼對應由包 3 定。
  2. **逐字稿**：每則 `id`／`role`／`text`／`raw`／`ts`／`opening`，欄名語意取 `TranscriptEvent` 子集（`src-tauri/src/data/scene/transcript.rs:22-75`）。
  3. **世界書觸發狀態**：照 ST 欄位名存旁檔，語意固定如下——
     - 條目識別：每條目一個穩定 ID（網頁版匯入卡時配發，存進存檔）；桌面版匯入重配 UID 時，旁檔附「穩定 ID→桌面 UID」映射表，兩邊都留。
     - 計時單位：照 ST，以「訊息則數」計（sticky／cooldown／delay 的剩餘值與起算位置），不用時間。
     - 目前回合位置：存最後一則已計入的訊息 `id`，匯入端據此知道計時算到哪。
     - 編輯／刪除／重新生成：照 ST 釘版本的實際行為（ST 把計時存在聊天中繼資料，回退時怎麼處理要在包 5 查原始碼確認，查證前不預設），查完寫成表格測試；契約只固定「每則訊息被移除或改寫時，旁檔要記得它造成的計時變化能否回退」這個欄位位置。
     本案桌面版只保存、不消費，也不把 sticky 條目改成 constant（桌面版 WI 補齊 ST 行為另案 [worldbook-st-trigger-parity](../tasks/worldbook-st-trigger-parity.md)，列為公開前門檻，D17）。
  4. **MVU 變數**：六層全列名（chat／character／global／preset／script／extension，空的也留）；每則的 `message_vars` 表；開場完整種子表；有效表選取規則（照「落地規則」的三條）；初始化巨集值；卡片 localStorage。
- 金鑰、模型設定不進存檔。
- v1 一開始就把四類欄位都定好；網頁版各包逐步填，桌面版匯入器從第一天就認全部欄位（用手寫 fixture 測）。
- 另有「匯出成 ST 聊天檔」選項（包 4，D3）：有損，只帶對話與卡，MVU 變數與世界書狀態在 ST 沒有對應位置；不是桌面版匯入格式，也不做從 ST 匯入。

### 桌面版匯入器的落地規則
- MVU：重建 `card-vars/control.json`（`data/message_vars/control.rs`：`mode=Events`、`macros`、該幕新 `epoch` 與種子）；每則 `vars_epoch` 對上新 epoch、`vars_rev` 全部新發；逐字稿整段寫一個 jsonl，不逐則走 `append_player_event`。卡片 localStorage 在桌面版是前端的 `card-storage:<worldId>`（`src/features/card-interface/useCardInterfaceController.ts:53`），匯入指令把內容回傳給前端，由前端寫進新桌 id。
- MVU 有效表三條規則（與契約第四類、包 3 驗收同一套）：
  - 開場種子用開場那張完整表。
  - 換幕後新幕面板用舊幕最後一張 `vars_epoch` 對得上該幕 epoch 的表，沒有就用該幕種子（`scene_seed_for_next`→`init_source`，`src-tauri/src/data/message_vars/write.rs:613-632`、`source.rs:37-74`）。
  - 沒寫進桌目錄的層（跨桌層依 D18 只補缺而未寫入的部分）以存檔裡仍在為準，不算丟。
- MVU 六層依桌面版實際落點分兩類（`src-tauri/src/data/card_vars.rs:34-36` 的 `in_world`、`:104-141` 的 `layer_path`）：
  - **桌內層**：chat（`worlds/<id>/card-vars/chat.json`，:112-114）、character（`card-vars/character/<雜湊>.json`，:116-121）、script（`card-vars/script/<雜湊>.json`，:125-130）。匯入時完整寫入並啟用，續玩就讀得到。
  - **跨桌層**：global（資料根 `card-vars/global.json`，:134）、preset（`card-vars/preset.json`，檔內 `id` 是 `"app"`，:135）、extension（`card-vars/extension/<雜湊>.json`，:136-141）。這三層所有桌共用，匯入只補桌面版沒有的鍵、既有值不覆蓋（D18）。
  - `<雜湊>`＝原 ID 的 SHA-256 前 32 個十六進位字元（`identity_name`，:76-82），原 ID 存在檔內 `id` 欄，對不上回 `IdMismatch`（:176）。
  - 匯入器一律呼叫既有的 `layer_path`／`write_layer`（:104、:385），不自己拼路徑。
- 容量鎖：匯入整段豁免 `check_capacity`（`src-tauri/src/scene_budget/mod.rs:341`），不自動分幕；滿了之後下一句由既有 `SceneCapacityFull` 擋，玩家走既有換幕 `advance_scene`（`commands/scene.rs:326`，新幕種子由 `message_vars::scene_seed_for_next` 產生，`data/scene/lifecycle.rs:140`）。換幕摘要一律由玩家操作觸發，匯入器不自行送摘要；摘要失敗可重試、原桌完整保留。理由：自動分幕會替玩家決定劇情斷點，也會替玩家花一次模型呼叫。
- 世界書可見性（D16）：匯入網頁存檔專用例外，卡內世界書條目設為該角色可見。相容規則：只套用在原卡**沒有**指定 `extensions.table_tavern.visibility` 的條目，原卡資料與明示的 visibility 原樣保留，不改桌面版全域預設（`data/worldbook.rs:718`）；世界書卡路（沒有角色 ID）不能一律設成 `Characters([id])`，另定並另測該路由。
- 失敗不留半桌，且跨桌層要能回滾（跨桌層寫在資料根 `card-vars/`，不隨新桌刪除）：
  1. 寫入順序：桌內全部（桌目錄、角色、世界書、逐字稿、桌內三層、`control.json`）成功落檔後，跨桌層才當最後一步寫；之前任何失敗只需刪新桌。
  2. 跨桌層寫入前記下受影響鍵的原值（journal）；寫入失敗或之後任何步驟失敗，就回滾到匯入前，再刪新桌。
  3. 回滾不得覆蓋匯入期間其他合法寫入：在既有 card-vars 的同檔鎖內做，或用既有 `rev` 版本 token 做 compare-and-set（`src-tauri/src/data/card_vars.rs:2-4`、`:191-195`），具體機制包 3 定。
  4. 匯入成功後到前端寫好卡片 storage 之前算未確認：journal 落在新桌 `web-save-pending.json`（每層寫入前先記），確認進桌才刪；玩家放棄走 `discard_web_save_import` 照 journal 撤回再刪桌，不走一般刪桌。

## 三、分包

順序：1 → 2 → 3 → 4 → 4b → 5 → 6 → 7 → 8 → 9。3（桌面版匯入器）排在 4（網頁存檔）之前：先用契約與手寫 fixture 把桌面版那頭做穩，4 再接真實網頁存檔做來回測試。包 3 先驗四類落地＋基本續聊，5、6 各自補自己那類的桌面來回測試，完整封板在 5、6 之後。

| 包 | 交付（可獨立驗收） | 建議 | 相依 | 風險 |
|---|---|---|---|---|
| 1 最小縱切 | 本機 `vite build`＋`vite preview` 點開→連 OpenRouter（PKCE＋貼金鑰）→自動挑免費模型→選內建範例卡（1 張）→串流聊天→頁首常駐「下載桌面版」連結（4.5 的 API，沒有正式版退 releases 頁）→今日免費次數→次數用完的導流面板（桌面版可接自己的訂閱＋下載鈕）。同包做：取消分兩種——「取消未完成回合」：尚未提交任何回覆時，連玩家那句一起自動收回、原文放回輸入框。這是網頁版提案〔模型判斷·未裁決〕，不是桌面版既有行為：桌面版零字停止只是不落事件、由玩家手動「收回」（`.ai/plans/ai-response-stop.md:4`），`src/features/play/useChatController.ts:865-883` 的自動收回只用在失敗。「停止並保留已產生內容」：已有正文或 MVU 更新就保留、標回應中斷，不刪玩家句以免留下孤立狀態，比照 ai-response-stop〔作者裁決 2026-10-01〕。停滯逾時與輸出上限（門檻照桌面版：首個進度 300 秒、之後 120 秒 `transport/stall.rs:12-14`；30,000 字元與空白退化 `transport/runaway.rs:8-14`）、送出互斥、取消後不寫入。換模：自動換下一支＋一次邏輯呼叫最多兩發（比照 `smart_free/call.rs:115`）；第二發只在第一發尚未輸出任何正文時才派（已吐正文不重送〔作者裁決 2026-10-04〕，`call.rs:155-160`），停止後不派第二發；不試打（D9）。含 2.2 的 repo 設定。存檔先只在記憶體。部署不在這包，D1／D12 不擋第一刀。 | Opus | — | 換模第二發會再用一次免費額度（失敗那發只是沒拿到內容，不保證沒計次）。 |
| 2 匯入卡 | 照卡片契約解析與分路（含世界書卡）、選開場白（first_mes＋alternate_greetings）、玩家名、重新生成／編輯／刪除最後一則、錯誤說明。提示組裝照 ST：system_prompt→description→personality→scenario→範例對話（`mes_example` 依 `<START>` 切段）→歷史（含 depth_prompt）→post_history_instructions；完整巨集引擎；送模前 regex（placement 含 1）與顯示 regex 分流。釘一個 ST 對照版本。卡片正規化檢視的 Rust／TS 兩邊測試與黃金檔在這包落地，並修桌面版 `import/card.rs:474` 的 `has_entries`，讓物件形 entries 兩邊同一規則。 | Opus | 1 | 完整巨集引擎與 regex 分流桌面版沒有，全新寫。 |
| 3 桌檔契約＋桌面版匯入器 | 寫定桌檔契約 v1；桌面版新增「匯入網頁存檔」（併進既有匯入、依檔案內容自動辨識，D13）：照卡片契約建桌建角（世界書路同步）、照二之二「落地規則」寫逐字稿、MVU、觸發狀態旁欄。驗收（測試通道＋假模型）：短 fixture 匯入→送一句、AI 回覆；長 fixture 匯入→下一句回容量滿→玩家按換幕（既有 `advance_scene`）→下一句送得出去，斷言換幕前後 MVU 完整表、桌內層（chat／character／script）完整延續、跨桌層（global／preset／extension）照 D18（未寫入的部分以存檔裡仍在為準）、卡片 storage 都沒丟、開場面板＝開場完整表、新幕面板＝舊幕最後一張對得上 epoch 的表，且 fixture 的換幕摘要本身不被容量擋；MVU 分開驗開場初始化、空表、尚無表；D16 的驗收直接比對下一輪送出的 messages，確認 constant 與命中 keyword 的條目真的進提示，不只檢查落檔；觸發狀態旁檔：驗完整保存，消費等 worldbook-st-trigger-parity；匯入失敗不留半桌；具名反例「第一個跨桌層鍵寫入成功、後續步驟失敗」→新桌不存在、跨桌層回到匯入前的值、匯入期間另一筆合法跨桌寫入保留。 | Opus | 2 | ①MVU 控制檔、`vars_rev`、`vars_epoch` 現有寫入路徑不支援整段帶入，要新寫入口。②世界書可見性：`data/worldbook.rs:718` 沒有 `table_tavern.visibility` 的條目預設給 GM，角色線（`transport/turns.rs:160`）濾掉；沿用 `import_character` 的話角色看不到自己卡的世界書（D16）。這可能也是桌面版一般匯入卡的既有缺口，要告知作者。③觸發狀態桌面版只存不用（另案）。④匯入成功、前端確認之前 app 崩潰：已裁決 B（D26）——新桌留著 `web-save-pending.json`、照一般桌出現，從桌清單刪桌不撤回跨桌層。〔作者裁決 2026-10-07〕⑤桌面版巨集只換 `{{user}}`／`{{char}}`，卡欄位的 `{{getvar}}` 在桌面版不會讀 chat 層（`card-vars/chat.json`）：桌面版既有限制，列給作者。 |
| 4 網頁存檔 | IndexedDB 多份存檔、persist 請求、Safari 提示、照契約匯出／匯入網頁存檔、匯出時附「用桌面版繼續」導流；另加「匯出成 ST 聊天檔」（有損，只帶對話與卡，D3）。驗收來回測試：網頁存檔→桌面匯入→同一桌能接著玩（測試通道）。現況：逐字稿進下一輪歷史；ST 聊天變數只落桌面版 chat 層檔案，桌面版的 MVU 表與提示內的變數等包 6〔模型判斷·未裁決〕。 | Opus | 2、3 | |
| 4b 上下文預算 | 照 ST 補 token 預算裁切（D23〔作者裁決 2026-10-07〕）：依實際派送模型的上下文上限扣掉保留輸出量當預算，估 token（照 ST 對該 API 的估算方式）；捨棄順序照 ST 預設（不釘範例，openai.js:1337）：固定段落（main、卡欄位、post_history_instructions）先佔，歷史由新到舊填到預算為止，剩下的才給範例對話——所以先掉範例、再掉最舊訊息；固定段落本身就超過上限時不送，給玩家提示（ST 的「Mandatory prompts exceed the context size」）。換模第二發照第二發模型的上限重算。驗收：表格測試對同一組逐字稿在不同上限下斷言送出的 messages。 | Opus | 2、4 | 估算與 ST 的實際 tokenizer 有落差；邊界附近可能仍被平台拒收。 |
| 5 世界書觸發 | 卡內 `character_book` 照 ST World Info：主鍵／正則鍵、次要鍵四種邏輯、掃描深度、大小寫與全字、constant、機率、遞迴、插入位置與順序、預算（沿用 4b 的 token 估算）、sticky／cooldown／delay、inclusion group（欄位多在 `extensions`）；觸發狀態照契約進存檔，補桌面來回測試。 | Opus | 2、4、4b | |
| 6 卡片介面 | regex 顯示腳本→整頁介面→沙盒 iframe（橋接照 2.4）；讀訊息墊片、MVU 讀寫墊片、parseMessage Worker、卡片 localStorage 墊片；宿主端變數語意改寫成 TS（4.1 的 message_vars 系列），照契約進存檔，補 MVU 卡的桌面來回測試。DRM／雲端載入器卡照桌面版回報不支援。 | Opus | 2、4 | 宿主端變數語意改寫成 TS 的工作量可能被低估；卡片腳本外連會外送對話內容（2.4）。桌面版巨集只換 `{{user}}`／`{{char}}`，卡欄位的 `{{getvar}}` 不讀 chat 層：網頁版變數接進桌面版提示要一起看（桌面版既有限制，列給作者）。 |
| 7 導流全套 | 下載頁（版本、各平台檔、Mac／Windows 未簽章繞過說明、功能對照表）、各時機的下載提示（D10）、額度用完文案只導向下載（D11）、找卡清單資料檔（五站連結＋一行 18 禁標示，D6）。 | Sonnet | 1、4 | |
| 8 十語系 | 十語系字典與字典體檢（比照 `check-i18n` 寫 web 版）、範例卡各語系（D5）、SEO／分享卡片的 meta。 | Sonnet | 1–7 | |
| 9 上線 | 部署 workflow、正式網域實開驗證（PKCE 回呼、CORS、CSP、下載連結）。只在「公開前門檻」全到位後做。 | Sonnet | 1–8；作者先開好 Cloudflare 帳號並接上 repo | |

### 公開前門檻（公開前要到位，不是開工前）
- 有可下載的桌面版 release：repo 目前零 release，`releases/latest` 回 404（4.5）；靠 [release-2-ci-windows](../tasks/release-2-ci-windows.md) 發出首個正式版。沒有 release 時下載鈕只能退 releases 頁，引流斷在最後一步。
- 介面卡兩案首發必含〔作者裁決 2026-10-04〕：[interface-card-panel](../handoffs/interface-card-panel.md)、[interface-takeover-spike](../handoffs/interface-takeover-spike.md)。網頁版導去下載的桌面版要已經帶這兩案。
- 目標 release 上，完整複合卡（世界書＋介面＋MVU）的網頁存檔匯入後能續玩。
- 桌面版 WI 補齊到 ST 行為：[worldbook-st-trigger-parity](../tasks/worldbook-st-trigger-parity.md) 做完才上線（D17）。〔作者裁決 2026-10-07〕
- 匯入卡的世界書角色看得到：[worldbook-character-visibility](../tasks/worldbook-character-visibility.md)。〔作者裁決 2026-10-07〕

## 四、查證結論

### 4.1 可沿用的底層

結論：前端純 TS 模組用路徑別名共用（D2）；Rust 的部分全部要用 TS 重寫；另有一批桌面版沒有、要照 ST 新寫。

**純 TS，可直接搬**
| 功能 | 位置 |
|---|---|
| PKCE（verifier／challenge、base64url） | `src/features/ai-connection/openrouter-onboarding.ts:33-64`（`createOpenRouterPkce` :57、`pkceChallengeForVerifier` :50） |
| OpenRouter 模型清單解析 | `src/features/ai-connection/model-catalog.ts:49`（`parseOpenRouterModels`）；公開清單直接 fetch `model-catalog-store.ts:41` |
| 金鑰格式檢查 | `src/features/ai-connection/api-key-check.ts:46` |
| AI 錯誤說明與遮蔽 | `src/shared/ui/ai-error.ts:65`、`:74`（依賴 `backend-text.ts` 的錯誤碼，要一起搬或改寫） |
| 故事 Markdown 渲染＋DOMPurify | `src/shared/ui/story-markdown.ts:73`、`:80` |
| ST 巨集名單（只是名稱表，不是引擎） | `src/shared/contracts/st-macros.json` |
| regex 顯示腳本→介面殼 | `src/features/card-interface/interface-card.ts`（`parseStRegex` :53、`applyScripts` :103、`extractShell` :143、卡片 storage 墊片 :184-200、IME 防護 :273） |
| 沙盒內建庫（jQuery／lodash／errorCatched） | `src/features/card-interface/card-sandbox-libs.ts:45` |
| 讀訊息墊片 | `src/features/card-interface/card-chat-shim.ts:46`、`:67` |
| MVU 讀寫墊片 | `src/features/card-interface/mvu/card-mvu-shim.ts:222`、`card-mvu-shim-source.ts:22`、`card-mvu-write.ts:140`、`card-mvu-parse-source.ts:9`、`card-mvu-parse-engine.ts:229`、`card-mvu-eval-host.ts:28` |
| iframe 沙盒寫法 | `src/features/card-interface/CardInterfaceOverlay.tsx:86-95`（`sandbox="allow-scripts"`＋srcDoc）——網頁版只能沿用 sandbox 屬性，不能照搬 srcDoc（會繼承宿主 CSP，見 2.4） |

注意：MVU 墊片的型別綁 `shared/contracts/backend-contracts.ts` 的 `TranscriptEvent`／`StateNode`；`card-shell-route.ts:6` 依賴重構的 `refactor-shell`（接管路線），網頁版只取原卡直玩那段。`useCardInterfaceController.ts` 全靠 Tauri `invoke`（:145-:580），要重寫宿主端。

**Rust，要用 TS 重寫**
| 功能 | 位置 |
|---|---|
| chat/completions SSE 串流 | `src-tauri/src/transport/client.rs:152`（`SseParser`）、`:334`（`extract_delta`）、`:353`（`StreamOutcome`）、`:639`（單支模型串流）、`:454`（HTTP 錯誤）；思考增量 `transport/runaway.rs:74`（`chat_reasoning`）——只搬 `extract_delta` 會把「想完沒有 content」當成功 |
| 失敗分類與 Retry-After | `src-tauri/src/transport/api_failure.rs:80`、`:193` |
| 上下文過長判定 | `transport/context_overflow.rs:10`、`:23` |
| 停滯逾時／失控輸出上限 | `transport/stall.rs:12-14`、`:38`；`transport/runaway.rs:8-14`、`:92` |
| 上下文預算 | `src-tauri/src/scene_budget/limits.rs`（`OUTPUT_RESERVE` :21、`resolve` :83） |
| 剝角色自加的「名字：」 | `transport/own_prefix.rs:6`、`:24` |
| `/key` 今日免費次數 | `src-tauri/src/smart_free/api.rs:145-188`（`free_model_daily_requests` 的 limit／remaining，缺欄＝付費帳號無上限） |
| 免費模型清單與排行 | `smart_free/api.rs:14`（`/models/user`）、`:30`（`top-weekly`）、`:44`（`category=roleplay`，不需金鑰）、`:61`（上游） |
| 選模與名單 | `smart_free/select.rs:25`、`:193`（`eligible_models`）、`:263`（`stable_candidates`）、`:330`（`build_lineup`，最多 4 支上游分散） |
| 失效換下一支 | `smart_free/failover.rs:14`（連續 2 次換）、`:16`（耗盡冷卻 30 分）、`:70`（`classify`）；一次邏輯呼叫最多兩發 `smart_free/call.rs:115` |
| PKCE 換金鑰 | `src-tauri/src/openrouter_oauth.rs:171`（`exchange_code`，打 `/auth/keys`）；桌面版回呼是本機埠，網頁版改回呼到站址 |
| PNG 卡解析 | `src-tauri/src/import/card_io.rs:26-60`（只認 tEXt；`chara` 優先、否則 `ccv3`） |
| 匯入分路與世界書剝取 | `import/card.rs:136`（`probe_import`）、`:458`（`worldbook_json`，`:474` 只認陣列 entries） |
| 開場白清單 | `src-tauri/src/import/card.rs:351`（`card_openings`） |
| initvar／規則抽取 | `src-tauri/src/import/mechanism.rs:148`（`import_mechanism`：`[initvar]` 初始樹、`[mvu_update]` 規則） |
| 世界書條目正規化與鷹架判定 | `src-tauri/src/data/worldbook.rs:687`（`normalize_imported_entry`，:718 預設 GM 可見）、`:733`（`is_mechanism_scaffold`） |
| 世界書觸發 | `src-tauri/src/transport/context.rs:8`——只做 constant＋最近 4 則子字串比對，**不是** ST 行為；網頁版照 ST 新寫（包 5），不照這支 |
| ST 巨集替換 | `src-tauri/src/transport/messages.rs:85`（只做 `{{user}}`／`{{char}}`） |
| 卡片介面抽取 | `src-tauri/src/import/interface.rs:129`、`:179`（`loads_mvu`）、`:204`（顯示腳本判定）、`:239`（雲端載入器卡） |
| MVU 變數語意 | `src-tauri/src/data/message_vars/`：`mode.rs`（模式交接）、`source.rs:58`／`:77`（初始化來源與有效表）、`turn.rs`（桌世代與回合紀錄）、`write.rs:569`／`:580`（開場表）、`convert.rs`；`data/card_vars.rs`（六層） |
| 匯出 | `src-tauri/src/data/scene/export.rs:180`、`:230`——都是給人讀的 Markdown，不能拿來回匯 |

**桌面版沒有、要照 ST 新寫**（包 2、5 釘一個 ST 對照版本）——對照版本：SillyTavern `06bde939`（2026-09-14，與 card-mvu-shim 同一版）；巨集照該版預設的新巨集引擎（`experimental_macro_engine: true`）。
- `mes_example` 依 `<START>` 切段。
- `system_prompt`／`post_history_instructions`／`depth_prompt`（卡片欄位覆蓋與插入深度）。
- regex 分流：送模前（placement 含 1＝使用者輸入等）與顯示用分開套。
- 完整巨集引擎（`st-macros.json` 只有名稱表）。
- World Info 完整觸發（包 5）。

### 4.2 找卡內建可行性
結論：五站都沒有「官方搜尋＋下載 API＋允許跨源」三者齊備的。**可內建只有 RisuRealm，而且只能做「玩家貼 ID 或網址→官方下載端點」，不能做搜尋或列表**；其餘四站只能放連結導流。查證時沒有金鑰、沒有登入，每站只發少數請求，CORS 用 Origin `https://example.com` 測。

| 站 | 公開 API | 條款 | 瀏覽器直連（CORS） | 18 禁欄位 | 分類 |
|---|---|---|---|---|---|
| RisuRealm | 有官方文件（https://realm.risuai.net/help/api）：只有 `GET /api/v1/download/:format/:id`（png-v2／png-v3／json-v2／json-v3；charx-v3 實測 403），**沒有搜尋**；文件明說不准用沒寫進文件的端點 | 禁爬取，但經文件化 API 取資料不在禁止範圍（https://sv.risuai.xyz/hub/tos）；每張卡自選授權，非商用且限制 API 的卡要帶 `non_commercial=true` 自行保證（https://realm.risuai.net/help/license） | 要帶 `?cors=true` 才回 `Access-Control-Allow-Origin: *`；預檢只放行 Content-Type、x-risu-api-version、accept；有限流 | API 沒有分級參數或欄位（內容規則要求標 nsfw，但 API 回應看不到） | 可內建（限貼 ID／網址） |
| Chub | 搜尋與下載都沒有官方文件（官方文件只有推論 API）；第三方逆向有 `api.chub.ai/search` | 查不到原文：台灣 IP 打 chub.ai 回 403「不提供服務」 | `api.chub.ai` 回 `*`，但台灣 IP 本體 403 | 逆向參數 `nsfw`／`nsfl`，受限卡要登入 | 只能放連結；另有台灣地區封鎖風險（台灣玩家是否同樣被擋查不到） |
| AICharacterCards | 查不到公開 API（只有前端內部端點） | 下載限流每分鐘 10 張，規避可停權；18 禁要年齡驗證（https://aicharactercards.com/terms） | 只放行自家網域 | 內部欄位 `nsfw`／`isNsfw` | 只能放連結 |
| Character Tavern | 查不到官方 API；第三方曾用的端點今日實測 404 | 明文禁止自動存取（https://character-tavern.com/legal/tos） | 卡圖 CDN 回 `*`，但條款禁止 | 站內 `warnings`，要登入 | 只能放連結 |
| Pygmalion | 查不到官方 API（前端內部 RPC） | 明文禁止爬蟲與資料探勘（https://pygmalion.chat/terms-of-service） | 只放行自家網域 | `includeSensitive` 要登入 | 只能放連結 |

完整查證紀錄（含非官方端點來源）在本次主線暫存，不進 repo；要用時以上表連結為準。

### 4.3 桌面版匯入網頁存檔
- 現況：桌面版**沒有**任何從外部檔案匯入桌次的入口。`restore_world_backup`（`src-tauri/src/commands/world.rs:217`）只還原 App 自己的桌備份；匯出兩支是 Markdown（4.1）。
- 桌面版一桌的落檔：`worlds/<ULID>/`，角色在 `characters/<id>.md`（卡欄位照介面語系拆段，`import/card.rs:20` 起的段標表），逐字稿 `transcript/<幕>.jsonl`，每則 `TranscriptEvent`（`src-tauri/src/data/scene/transcript.rs:22-75`），MVU 控制檔 `card-vars/control.json`。
- 網頁存檔格式草稿（自訂 JSON，D3；正式定稿在包 3，以「二之二、桌檔契約」為準）：
  ```json
  { "format": "table-tavern-web-save", "version": 1, "exported_at": "...",
    "card": { "spec": "chara_card_v3", "spec_version": "3.0", "data": { ... }, "...未知欄位": "原樣" },
    "card_png": "<原 PNG base64，可省>", "import_route": "character|worldbook",
    "user_name": "...", "opening_index": 0,
    "messages": [ { "id": "<ULID>", "role": "user|char", "text": "...", "raw": "...",
                    "ts": "...", "opening": true, "message_vars": { ... } } ],
    "wi_state": { "<條目穩定 ID>": { "sticky": 0, "cooldown": 0, "delay": 0 } },
    "mvu": { "seed": { ... }, "macros": { ... }, "selection": "...",
             "layers": { "chat": {}, "character": {}, "global": {}, "preset": {}, "script": {}, "extension": {} } },
    "card_storage": { ... } }
  ```
- 匯入端走既有 `import_character`（`src-tauri/src/commands/character.rs:80`）或世界書路（依 `import_route`），段標、世界書、介面腳本照桌面版現有匯入規則處理。

### 4.4 部署形態
- `scripts/check-structure.mjs:12` 只走 `src/`，`web/` 不會被擋，但也完全不受檢查；要照 2.2 擴充。
- 根 `tsconfig.json` 的 `include` 只有 `src`，不受影響。根 `vitest.config.ts` 用預設 include＋只排除 `*.webkit.test.tsx`、`.claude/**`，**會**收進 `web/**/*.test.ts`，要排除。
- `verify.yml` 只在 `.ai/**`、`**.md` 以外有改動時跑，`web/` 改動會觸發；目前只 `npm ci` 根目錄。
- 桌面版 CSP 是 `null`（`src-tauri/tauri.conf.json:20-21`），網頁版 CSP 照 2.4 另定。
- 託管候選（只列事實）：
  - GitHub Pages：站點上限 1 GB、軟頻寬 100 GB／月、軟建置 10 次／時（自訂 Actions 不受此限）、部署逾時 10 分；條款禁止當商業交易或 SaaS 的免費主機。官方限制頁沒提自訂 HTTP 標頭（CSP 只能用 meta）。repo 已公開。來源 https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits
  - Cloudflare Pages 免費方案：500 次建置／月、單站 20,000 檔、單檔 25 MiB、100 個自訂網域、`_headers` 檔可設標頭（100 條）；官方限制頁沒列頻寬上限。來源 https://developers.cloudflare.com/pages/platform/limits/
- OpenRouter OAuth PKCE 回呼可用任何公開 https 網址，流程可純前端完成，授權碼 10 分鐘過期。來源 https://openrouter.ai/docs/use-cases/oauth-pkce

### 4.5 桌面版版本與下載連結
- 桌面版更新走 tauri updater，端點寫死在 `src-tauri/tauri.conf.json:44-46`：`https://github.com/TaoGongSun/Table-Tavern/releases/latest/download/latest.json`；回退點的下載網址由同一端點推出（`src-tauri/src/updater/rollback_point.rs:30-75`）。`latest.json` 由發版 workflow 組（`.ai/plans/desktop-update-detect.md:61-84`）。
- **網頁版不能同一套**：`github.com/.../releases/latest/download/...` 回 302 且沒有 `Access-Control-Allow-Origin`（以 `cli/cli` 實測），瀏覽器 fetch 會被 CORS 擋。
- 改打 `https://api.github.com/repos/TaoGongSun/Table-Tavern/releases/latest`：回 `Access-Control-Allow-Origin: *`、未帶金鑰每 IP 每小時 60 次、`cache-control: max-age=60`；回應有 `tag_name` 與 `assets[].browser_download_url`，下載鈕直接用這個網址當一般連結（導覽不受 CORS 限制）。資產檔名固定為 `TableTavern_<版本>_x64-setup.exe`、`_aarch64.dmg`（desktop-update-detect 計畫 :82）。
- 目前 repo 沒有任何 release，`releases/latest` 回 404：網頁版要退回 releases 頁連結；GitHub 的 latest 不含預發布與草稿。
- 每個資產的 `download_count` 是公開欄位，不必加追蹤就能看下載數。

## 五、作者拍板（2026-10-07）

全部〔作者裁決 2026-10-07〕。
- **D1 託管**：Cloudflare Pages。理由：`_headers` 能按路徑設標頭，宿主與卡片沙盒的 CSP 才分得開。作者開帳號接 repo，到包 9 才需要。
- **D2 底層沿用**：路徑別名共用桌面版純 TS 模組，verify 兩邊一起跑。理由：修一次兩邊都有。
- **D3 存檔格式**：自訂 JSON 為桌面版匯入格式；另加「匯出成 ST 聊天檔」選項，排進包 4。理由：桌面版匯入要帶得過 MVU 與觸發狀態；ST 匯出有損（只帶對話與卡），不做從 ST 匯入。
- **D4 卡片介面**：只做原卡直玩。理由：重構不進網頁版。
- **D5 範例卡**：新寫一張單角色範例卡。理由：單張卡是主流。
- **D6 找卡**：五站只放連結；RisuRealm 貼網址匯入日後另案。理由：不碰站方 API。
- **D7 金鑰**：存 localStorage＋登出鈕。理由：下次打開免重新授權。
- **D8 選模**：只自動選模。理由：零設定。
- **D9 試打**：不試打。理由：不額外佔每日免費次數。
- **D10 下載提示**：固定節點提示（額度用完、匯出存檔、碰到桌面版才有的功能），加上頁首常駐、隨時可點的下載連結——常駐連結是裁決內容，不是可省細節。理由：干擾少、隨時找得到。
- **D11 額度用完文案**：只導向下載。理由：引流到桌面版。
- **D12 部署**：手動觸發 workflow。理由：CI 打包等作者說了才觸發。
- **D13 桌面版匯入入口**：併進既有匯入、依檔案內容自動辨識。理由：玩家不用找新按鈕。
- **D14 首發語言**：十語系。理由：與桌面版一致。
- **D15 反向通用**：不做桌面→網頁。理由：桌面桌一桌多角色、GM、多幕、狀態樹，對不回一桌一卡。
- **D16 匯入網頁存檔的世界書可見性**：匯入網頁存檔專用例外，條目設為該角色可見（相容規則見二之二落地規則）。理由：與玩家在網頁版的體驗一致。
- **D17 桌面版 WI 補齊**：等 worldbook-st-trigger-parity 做完再上線，列公開前門檻。理由：兩邊觸發行為一致才算續玩。
- **D18 跨桌 MVU 層衝突**：只補缺，既有鍵值不覆蓋（補進原本缺少的共享鍵仍可能改變其他桌讀到的值）。理由：不改掉玩家其他桌既有的值。

### 包 2 拍板（D19–D23）
全部〔作者裁決 2026-10-07〕。
- **D19 網頁版不收的檔**：獨立世界書檔、沒名字或名字有換行的卡給說明不收，請玩家改用桌面版。理由：網頁存檔一定要帶得回桌面版。
- **D20 卡檔大小上限**：30 MB，先看檔案大小、不讀內容。理由：擋掉誤選的大檔、不吃光記憶體。
- **D21 卡內 regex 腳本**：照 ST 匯入時問一次——匯入預覽有「允許使用」開關，預設照 ST 不允許（regex/index.js 未同意前不套）；不允許就整組不套（送模前、顯示、玩家輸入、編輯都不套），結果存在 `PlayCard.regexAllowed`，網頁存檔要帶。理由：玩家知情，惡意卡的 regex 不會不經同意就跑。
- **D22 重新生成**：沒拿到新回覆（取消或失敗）就放回原回覆，開場白不能重新生成。理由：不丟資料。
- **D23 上下文預算**：照 ST 補 token 預算裁切，獨立成包 4b（排包 4 後、包 5 前，不擋包 3）。理由：預算與世界書觸發各自可獨立驗收，包 5 的 WI 預算直接沿用 4b 的估算器，不讓包 5 過大。
- **D24 拒絕 regex 的存檔匯進桌面版**：該桌不跑卡的 regex 腳本（桌層旗標，一般匯卡照舊）。理由：尊重玩家在網頁版的選擇。〔作者裁決 2026-10-07〕
- （`{{pick}}` 已移植 seedrandom，與 ST 位元相同。）

### 包 3 拍板（D25–D26）
全部〔作者裁決 2026-10-07〕。
- **D25 匯入網頁存檔不記收據**：一律開新桌，不要就從桌清單刪桌（跨桌層補進去的鍵不跟著退）。理由：整桌由存檔建出，刪桌就是復原，不必另一套收據。
- **D26 匯入成功、確認前崩潰不另處置**：那張桌照一般桌出現、可以照玩，`web-save-pending.json` 閒置；從桌清單刪桌不撤回跨桌層。理由：那些值是玩家自己存檔裡的補缺值，留著不算汙染。

### 包 4 拍板（D27–D29）
全部〔作者裁決 2026-10-07〕。
- **D27 自動存檔何時佔格**：新開的桌等玩家第一次開口才佔一格（開口那一刻先存開口前的樣子）。理由：只看看不玩的卡不留一堆空存檔。
- **D28 串流中重新整理**：退回上次完整回合，半截回覆丟掉、玩家那句放回輸入框（草稿記在 sessionStorage，只跟分頁、不進存檔）。理由：存檔永遠是完整回合，玩家那句也不會不見。
- **D29 ST 的跨對話 global 變數**：照 ST 存進瀏覽器（IndexedDB，與存檔同庫另一個 store），所有存檔共用、重新整理不丟；從存檔接著玩時只補缺，匯出只寫這格原有的鍵＋本桌寫過的鍵，衝突鍵以這格為準。理由：與 ST 行為一致，存檔進出不丟別桌也不混別桌的值。

## 六、驗收方式（候選，未安裝）

網頁版沒有測試通道，改用以下機械驗證：
- **vitest（web/ 自己一份）**：純邏輯——卡片解析、世界書觸發（照 ST 行為的表格測試）、SSE 解析（含只有思考沒有 content 的串流）、選模與換模、存檔格式來回、巨集、匯出無金鑰斷言。
- **Playwright 端對端**：根目錄已有 `playwright` 1.55.1 與 `@vitest/browser-playwright`（`package.json` devDependencies、`vitest.webkit.config.ts`）。對 `vite preview` 跑 Chromium＋WebKit；OpenRouter 用 `page.route` 攔截回假 SSE（格式可比照 `scripts/harness-fake-openrouter.mjs`），零額度驗：授權回呼（含 state 不符、取消、重整不重換）、串流、停止、停滯、換模、額度用完導流面板、下載連結、匯出存檔、卡片介面 iframe 內按鈕寫入 MVU、舊 iframe 與外部視窗偽造 postMessage 被丟棄。
- **卡片契約黃金檔**（驗收項）：同一張複合卡由 Rust 測試與 web vitest 各算出「卡片正規化檢視」，對同一份黃金檔比對；CI 用進 repo 的自製複合卡，本機加跑 `TestCards/` 實卡（含世界書卡，驗分路不漏世界書）。
- **桌檔來回測試**（驗收項）：包 3 用手寫契約 fixture、包 4 起用網頁版真匯出的存檔，cargo 測試與桌面版測試通道（`node scripts/harness.mjs`）匯入後核對逐字稿、世界書、MVU 變數，並照包 3 驗收項（短／長 fixture、MVU 三種情況、失敗不留半桌）送句確認能接著玩。
- **測試用模型**〔作者裁決 2026-10-07〕：桌面版那頭（包 3 匯入後續玩、來回測試）用測試通道＋claude CLI Sonnet；網頁端機制測試用本機假端點（比照 `scripts/harness-fake-openrouter.mjs`）；OpenRouter 金鑰目前無額度（403），真免費模型實送等有額度再補，每包報告註明「真模型未實送」。
- 上線後：部署網址實開一次，確認 PKCE 回呼網址、CORS、CSP、下載連結在正式網域都通。
