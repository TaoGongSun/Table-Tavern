# Handoff: refactor-survey-spans

## ⚠ 對話紀律（2026-08-12 使用者嚴令）
- **驗 Rust 改動先認 Compiling 行**：這台機器 tauri dev 冷啟動不重編 Rust（cargo fingerprint 誤判 0.2–0.5s Finished 沿用舊 binary），只有 watch 偵測檔案變更才真的編（會印 Compiling）。改完 Rust 要嘛看到 watch 的 Compiling＋relaunch，要嘛 survey 快取 miss（hit<100%）才算新碼上場——13:21／13:30 兩輪都是舊 binary 白跑。
- **禁止長思考**：驗收＝一項一條指令一行過/不過；查帳題直答；異常先一行列出問使用者要不要追，禁自行開挖（uid 考古、快取歸因這類旁支全禁）。思考超過一分鐘會被打斷。
- 拍板請求必附：問題主體＋成因＋各選項後果；沒問題的項目至多一行。
- 大查詢派新對話或代理，別堆本對話 context。

## ⚠ 介面軌重大裁決（2026-08-12 17:30，蓋過下方 T3 介面相關驗收項）
T3 毛絨實測後使用者判定：**interface_shell 產殼路線完全不合格**——模型發明新介面＋GM 輸出格式變質＝卡變成另一張卡。新規格：介面渲染永遠照搬原卡模板（regex_scripts 的 replaceString），AI 永不發明介面；省額度靠 app 組 XML 餵殼。生死由 [interface-takeover-spike](../tasks/interface-takeover-spike.md)（西幻卡三步實測）決定：全過＝介面接管重寫、任一不過＝介面軌刪除只留人物＋世界書。本案 T3/T4 驗收照舊，但介面相關項（毛絨殼、美化状态栏拆分）不再是結案判準。

## Current state（2026-08-12 16:00，T2 第三輪驗畢：B 過／A 擱置觀察／C 待 T3）
三案（A 人物判準防重寫、B absorb 直給樣式、C 判官理由落檔 excused）已實作 commit eed37f2。15:39 重跑镇北王府（survey opus out=5,444 hit=0% 快取重建＋pool 12 筆 sonnet，共 ≈$2.14、6分45秒）結果：
- **B 過**：三條「剧情-」全數 kind=mechanism＋App 接管中；③格式增强PLUS drop①、④美化状态栏拆介面欄位維持過。
- **A 不過→使用者裁示擱置**：8 人仍 tangled 重寫（pool 12=8 人＋3 absorb＋1 介面）。裁示：**不再加壓 prompt**（自創 ST 卡混寫本來就多，加壓過頭易錯判），只加診斷水印、繼續測其他卡看成效。已在 lib.rs refactor_survey 收尾加 `[survey-persons]` eprintln（name/mode/uids/spans，分辨「沒寫 mode」vs「明判 tangled」，隨 43dde2b 入庫）——**驗完即刪**。
- **C 路徑就緒、本輪無畫面＝正常**：三條剧情直接接管→無 carry 需開脫→audit=[]→稽核節整個不顯示。C 的 UI 實測落在 T3 年表 carry+reason 項。
- **卡片介面空白已根治（2026-08-12 17:00，毛絨桌實測恢復）**：三次幽靈空白（結果視窗、全桌介面、毛絨桌持續壞）root cause＝雙格＋onLoad 翻面雙緩衝在 WKWebView 不可靠（devtools 水印實證：帶 script 的 srcdoc iframe load 事件不 fire、塞格 setState 無聲失效，殼一路健康到狀態機門口被弄丟、front 永遠指著空格）。修法＝狀態機整台拆除，單 iframe 直繪＋key 綁殼指紋（App.tsx，vitest 108／build 綠）。代價（殼更新閃白）已立案 [shell-update-flash](../tasks/shell-update-flash.md)（postMessage ready 重建雙緩衝，未排程）。

### 環境陷阱（驗收必讀）
- **供應商隔離只有 claude／codex 有**（2026-08-14 實證）：claude 帶 `--safe-mode`（官方定義：CLAUDE.md／skills／plugins／hooks／MCP／自訂指令全關）、codex 帶 `--ignore-user-config`＋`--ephemeral`；`grok_args`／`agy_args` 無等價旗標（grok 只關工具：`--deny *`／`--disable-web-search`／`--no-plan`／`--no-subagents`），grok 還會寫 `~/.grok/sessions`／`memtrace` 跨會話記憶。當天兩輪盤點誤設成 grok，模型從「盤點世界書」滑成「跟操作者交涉」（畫面吐「沒拍板前不會輸出半成品盤點」「停鉤仍失敗」），**不報錯、直接產爛盤點**。補隔離（grok `--no-memory` 等）延後到重構按鈕做完。
- **proxy usage 罐頭**：prompt_tokens/cached_tokens 恆報 28492/28486（跨不同 prompt 與 world）＝不可信，禁拿它判 prompt 新舊；驗 prompt 生效看 survey out 量級變化或臨時 eprintln 水印。
- balanced 檔位曾指死模型 claude-sonnet-4-20250514（秒死 out=0），使用者已換 sonnet-4-6 可用。


2026-08-12 **T1 全過收檔**（第三輪 11:42 實測）：①時長使用者裁決滿意（survey 11:42:43 快取 100% 中→pool 完 11:44:43 共 2 分，總成本 $0.67 vs 首輪 $0.86）②③首輪過 ④放回實點過（3→2 條、進條目區勾選、outcome 檔同步）⑤audit=[] 紅字歸零＋Rigurd (EN VER.) 標④ ⑥盤點中心跳點顯示。跨模型快取 miss 議題收掉不調（成本 ~$0.05/輪，使用者裁示不糾結）。
當日三批修（62695e9／63100e1／本批）：
- **放回標題去段標（B 拍板，本批）**：restoreDropped title 一律用原標題，內部 ⟦sN⟧ 不露出（refactor-review.ts，vitest 108 綠）。
- **思考字尾心跳點（A 拍板，63100e1）**：4.7 世代 CLI 隱去思考本文（`thinking:""` 只剩 estimated_tokens）→cli.rs 空思考增量轉「⋯」，約每 50 tok 一顆。
- **CLI 死法全收網（63100e1，跨平台 tokio API）**：①程序退出但管線不 EOF→800ms 強制收尾 ②斷流→120 秒無行＝殺程序＋⚠ 字尾回錯 ③stdin 餵不進→60 秒逾時 ④crash 無收尾事件→報「CLI 異常結束＋stderr 尾巴」，殘缺正文不靜默往下走（新測試把關）。第二輪 11:15 卡死事故（CLI 消失、app 卡「盤點中」、dev 終端先死成孤兒）由此收網。

### T2 镇北王府首輪（2026-08-12 12:18，opus 4.7）：①八人一人一卡過；②③④全不過——survey 只吐 620 tok 敷衍（sonnet 兩筆 out=0 待查），零 absorb 零 drop，美化状态栏／格式增强Plus 判官塞進人物 uids 名義下落、實際無產物、無聲殘留。兩刀已修（cargo 476 綠）：
- **預掃改語言無關結構特徵（拍板）**：模板變數（排除 {{user}}/{{char}}）、表格 ≥3 行、代碼塊/HTML 標籤、百分比數值＋原有逐日；trigger:/rule: 詞彙降為免費加分。中文卡機制條目 carry 無 reason 從此會亮機制守恆紅字。
- **涵蓋稽核收緊**：clean 人物只認 spans/private 實際引用的 uid，uids 欄多列的不算下落→漏網補 carry＋紅字（镇北王府殘留洞的根治）。
- **absorb 判準擴事件劇本（A 拍板）**：survey prompt「觸發條件（數值）」改「觸發事件（數值、情境都算，trigger:/condition:＋演出劇本）」，歷史年表分界加「已經發生過的」——「剧情-」類事件劇本從此該判 absorb。改 user 訊息端，同卡重跑首輪 survey 段快取 miss（幾毛錢級）。reason 存檔顯示（B 案）記待辦不擋驗收。

前一輪三修（已 commit 62695e9）：
1. **淘汰理由④內容重複**：語言重複版（Rigurd EN VER.）以前被 survey 指示「不要列」→必觸發漏網紅字；現在通用規則「重複內容取一捨餘」，棄用版標 `drop rule: 4` 進淘汰清單。改點：refactor_ai.rs prompt（PERSONS＋drop 四種＋SPLITS）、refactor_assemble.rs 範圍 1–4、App.tsx rule map、十語系 +refactorDroppedRule4。
2. **502 發呆三分鐘**：CLI「API Error…」走 stderr、以前收尾才讀。現在 run_cli stderr 逐行讀：錯誤行立即進進度字尾（⚠ 前綴）；設定類（unknown provider/401/404…）立即殺程序回錯（api_error_kind 分類）。
3. **環境劫持自清**：run_cli 起子程序前拔掉所有繼承 `ANTHROPIC_*`（四家 CLI 全蓋），接閘道唯一通道＝app 設定 claude_base_url。根因：老分頁殘留 Sol 代理 export（BASE_URL=127.0.0.1:8317＋cpa_ token），環境本身的清理另開對話處理（提示詞已交使用者）。

### T1 首輪結果（兽人的洞穴 …ESK6MN，2026-08-11 20:58–21:03）
- 4 呼叫：survey opus 8,612 tok＋sonnet 並行 783/3,216/14,507；$0.86；jsonl 首末筆 3分37秒，含 survey 生成全程 **≈5.5–6 分（超標 <5 分）**，超標主因＝survey 輸出量＋並行③ 14.5k tok 長尾。舊制同卡 30.6 分／17 筆／$1.85。**時長要不要追（prompt 層瘦身）待使用者拍板**。
- ② byte 相等 9/9＋constant 保留 ✓ ③ 接管 kind=mechanism＋規則＋6 觸發器＋帳本 ✓ ④ 淘汰 rule2 ✓（放回鍵未實點）⑤ 漏網紅字 1（＝問題 1，已修待重跑驗）。
- 快取觀察：survey=opus、pool=sonnet 跨模型快取不共用，並行齊發 3 筆中 2 筆 miss（多 ~$0.05/輪，時間無損）。要不要調待拍板。

## Completed
- **包5**（sonnet 代理＋主線複驗，vitest 105→108，隨包 5 commit）：結果面板三新區——已淘汰（收合、逐項 rule 標籤＋展開全文＋「放回」走 restoreDropped 純函式轉 carry 進 entries 並勾選，App.tsx:2948 起）、未接管機制清單、稽核紅字；i18n 十語系各 +12 鍵。
- **包4**（sonnet 代理＋主線複驗，vitest 94→105、build 0，commit bded948）：runAiRefactor 整段重推（App.tsx:2115）——survey→refactor_assemble_local→人物/absorb/group/statusbar/interface 全並行 pool（chain 恆空、warmed:true 直進並行，refactor-run.ts 加 warmed 選項）；knownFields＝survey.fields 固定一份；產物合併含 local＋dropped/unabsorbed/audit 透傳；刪 RefactorPlanEntry／rewrite 分支／knownFields 累積。
- **包3**（sonnet 代理＋主線複驗，cargo 463→470，commit 978d46c）：absorb_messages/parse_absorb（本文照搬鎖定、AI 只出 RULES/TRIGGERS、`{{span:uid#sN}}` 指位）＋group_messages/parse_group（GroupKind、大組 >4000 字保險）＋expand_span_placeholders；lib.rs 三新 command refactor_absorb_entry（Rust 端組完整條目含 meta）/refactor_split_group/refactor_expand_spans（statusbar 段走 interface 型）；刪 PlanKind/RefactorPlanEntry/rewrite_messages/parse_rewrite/refactor_rewrite_entry。
- **包2**（sonnet 代理＋主線複驗，cargo 455→463，commit bc3eef7）：新模組 [refactor_assemble.rs](../../src-tauri/src/refactor_assemble.rs)（assemble_local:66——carry 含元資料 byte 相等、split 七路由組裝＋餘段兜底、clean 人物組卡、四項稽核 coverage/mechanism/split/drop_rule）；RefactorEntryMeta＋RefactorNewEntry.meta（refactor_ai.rs:1383）；apply() meta 雙軌落檔（refactor.rs:286，meta.order 不吃遞增）；RefactorOutcome 加 dropped/unabsorbed/audit 三欄；lib.rs:603 command refactor_assemble_local。
- **包1**（sonnet 代理＋主線複驗，cargo 442→455）：EntrySpan/segment_spans（refactor_ai.rs:103/112，空行切段、byte 相等 property 測試）＋⟦sN⟧ 標記（mark_entry_spans:164）＋PrescanSignal/prescan_worldbook（:215/:238，trigger:/rule:/逐日 regex）＋新 SURVEY_BODY 六區塊（:313）＋survey_messages 帶 signals（:420，lib.rs:569 接線）＋新 structs（RefactorSurveyPerson 擴充 mode/spans/private_spans、RefactorEntryVerdict、RefactorSpanRoute、RefactorSplitGroup、RefactorSurveyOutcome 刪 plan 改 verdicts/splits/groups/fields）＋六區塊解析器（locate_fields 固定欄序通用抽取）。RefactorPlanEntry／rewrite 舊中段保留待包 3 刪。

## 盤點結論（2026-08-11，主線親查）
- **18k 對號**（~/Documents/TableTavern/prompt-cache.jsonl，14:21–14:46 run）：14 筆＝survey（opus-4-7，3705 tok）＋條目重寫 11（5 設定小筆 0.5–4k＋6 機制大筆 7–18k）＋介面 1（2542）＋人物展開 1。**兩筆 18k＝機制條目重寫，非介面**；重寫可見正文僅 ~0.5–1.2k 字/條→輸出 token 大宗＝思考＋JSON 開銷。結論：照搬消滅整筆呼叫（含思考）是提速主力；介面軌無大額，維持「無介面卡不生介面」。
- **快取紅線**：[refactor_ai.rs](../../src-tauri/src/refactor_ai.rs) 測試 `all_stage_system_messages_are_byte_identical_for_same_context` 把關跨階段 system 相等；階段指示都在 user 訊息端。`assemble_card_context` 為重構專用（遊玩線走 transport.rs），context 內加 span 標記不影響遊玩、不破跨階段相等。
- **認人沿用**：PERSONS 區塊＋buildRefactorPersonPlan（單一專屬來源本地轉換）＋person_expand＝person-promote 實作，本案只擴充欄位、不做第二套認人。
- **IR 就緒**：MECHANISM_SCHEMA（FieldRule＋Trigger）→ RefactorNewEntry.rules/triggers → apply() 併 state.mechanism＋locked＋機制帳本（refactor.rs:272–303），接管線直接沿用、不重造。
- **apply() 現況**：新條目一律 keys=[]、constant=false（refactor.rs:279）→ 照搬路徑必須補元資料保留。

## 小抄合約 v1（包 1–5 依此實作）

### Span 基礎建設（app 端）
- 條目內容以「空行」切 span（段）：span＝原文 byte 區間，依序串回（含原分隔空行）必得原文 byte 相等。`format_worldbook_entry` 每段行首加標記 `⟦s1⟧`（各條各自從 s1 起編）；引用寫法 `uid#sN`。
- 結構預掃：app regex 掃各 span（不分大小寫）：`trigger:`／`rule:`／逐日樣式（`第[一二三四五六七八九十\d]+天`、`每日`、`\bday ?\d`）→ 訊號清單隨 survey user 訊息注入（「結構預掃訊號：uid=3#s2 含 `trigger:`」）。判官判定與訊號衝突（含訊號條目蓋 carry）必附 reason 一句。

### survey 輸出六區塊（新 SURVEY_BODY，只動 user 訊息端）
```
## PERSONS
- name: 霍玄 uids: 12,45 player: yes mode: clean spans: 12#s1,45#s2 private: 45#s3
- name: 阿蘭 uids: 8,9 mode: tangled
## INTERFACE
- uid=201 playable: no
## ENTRIES
- uid=3 action: carry
- uid=4 action: carry reason: 歷史年表非機制
- uid=9 action: absorb
- uid=5 action: drop rule: 2
- uid=7 action: split
## SPLITS
- span: 7#s1 route: statusbar
- span: 7#s2 route: gm
- span: 7#s3 route: drop rule: 1
- span: 23#s2 route: person name: 霍玄
- span: 23#s4 route: entry title: 王府概況
- span: 16#s2 route: group id: g1
- span: 16#s6 route: unabsorbed note: 擲骰檢定
## GROUPS
- id: g1 title: 格式與行為 kind: mechanism spans: 16#s2,16#s5,18#s1
## FIELDS
- 好感度
- 淪陷天數
```
- PERSONS／INTERFACE 沿用現行格式；PERSONS 新增選配欄（固定順序 name→uids→player→mode→spans→private）：`mode: clean`＝所列 span 原文組裝成卡（private 列私密段、其餘公開，零呼叫）；`mode: tangled`＝一人一次 balanced 呼叫（現行 person_expand）；mode 缺席＝沿現行分流。
- ENTRIES：每條非純人物條目一行，action 封閉字彙 `carry|absorb|drop|split`（照搬｜接管｜淘汰｜需拆）。drop 必附 `rule: 1|2|3`（①輸出容器紀律 ②版本標記/更新日誌 ③ST 引擎專屬鉤子）。純介面格式條目走 INTERFACE 即可；混寫的走 split。
- SPLITS route 封閉字彙：`statusbar`（欄位綱要＋顯示條件→該條目一次 interface 型呼叫抽 STATE，emoji 標籤留值裡）｜`gm`（敘事行為指令→原文組裝進 GM 規則條目，零呼叫）｜`drop rule: n`｜`person name: X`（X 必為 PERSONS canonical 名）｜`entry title: T`（設定段→同 title 段原文串接成新條目，零呼叫）｜`group id: gN`｜`unabsorbed note: …`（app 尚無機構的機制→原文組裝進 GM 規則條目＋列「未接管機制」清單）。
- FIELDS＝命名權威一次頒布＝knownFields 唯一來源；執行端未列名者照原文沿用、禁自創，產物欄位名事後機械 dedup。

### 執行端呼叫（全並行上限 4，序列鏈廢除）
- absorb：一條一次 balanced（新 absorb_messages）：入＝該條全文（含 span 標記）＋FIELDS＋MECHANISM_SCHEMA；出＝僅 `## RULES`＋`## TRIGGERS` JSON——**無 CONTENT，條目本文原文照搬＋locked**。trigger text／preamble 可寫 `{{span:uid#sN}}` 指位，app 組裝時替換原文。
- group：一組一次 balanced（新 group_messages）：入＝組內 span 原文＋FIELDS＋目標 schema；出＝kind=setting→`## CONTENT`；mechanism→CONTENT＋RULES＋TRIGGERS。大組保險：組源合計 >4000 字時 prompt 指示可用 `{{span:…}}` 指位照搬、只重寫真糾纏句。
- statusbar：per split 條目一次現行 interface 型呼叫（material＝該條 statusbar spans）。tangled person＝現行 refactor_expand_person。INTERFACE uid＝現行不變。
- 首發快取由 survey 本身建立（同 run 先行），pool 全並行直接開跑。

### 零呼叫組裝＋機械稽核（Rust，新 command refactor_assemble_local）
- carry：來源條目→產物條目，content byte 相等＋keys/constant/order/visibility 原樣（RefactorNewEntry 加選配 meta 欄，舊產物 JSON 不帶 meta 照舊可解）。
- clean person／entry title／gm／unabsorbed：span 原文串接組裝；gm 條目 title＝原標題（撞名才後綴 " (GM)"）。
- dropped：Vec<{uid,title,content,rule}> 全文入清單。
- 稽核四項（audit report，玩家可見）：
  1. 涵蓋：每 uid 必有下落（PERSONS/INTERFACE/ENTRIES 其一）；漏網→自動補 carry＋報告列出。
  2. 機制守恆：預掃訊號 span 必落 absorb/statusbar/gm/group(mechanism)/unabsorbed；落 carry 且無 reason→報告紅字＋列未接管機制清單（原文仍保留，資料不掉）。
  3. 拆組守恆：split 條目每個 span 必有 route；漏→補進「<原標題>（餘段）」carry 條目。carry byte 相等由「slice 原文組裝」保證＋測試斷言。
  4. 淘汰稽核：drop 缺編號或編號不在 1–3→自動退回 carry。

### 產物與 UI
RefactorOutcome 擴充：entries[].meta＋dropped[]＋unabsorbed[]＋audit[]。淘汰清單 UI：預設不套用、收合列表、逐條展開看全文、一鍵放回（轉 carry 進 entries 並勾選）。未接管機制＝資訊列表（內容已在 GM 規則條目裡）。

## 實測清單（新對話實機驗收用）
**2026-08-14 拍板：T1–T3（卡片盤點品質）全部擱置到重構按鈕做完再測**——做完＝[refactor-mode-split](../tasks/refactor-mode-split.md) 兩段式定向落地＋[interface-takeover-spike](../tasks/interface-takeover-spike.md) 介面接管收尾。管線還要改，現在測產物品質等於拿舊管線當判準。**今天只跑 T4。**

環境：先重編譯再驗（裸 `npm run tauri dev`，app 已自清 ANTHROPIC_*）。時間帳看 `~/Documents/TableTavern/prompt-cache.jsonl`（ts＝完成時刻，diag single＝重構呼叫）。世界目錄 `~/Documents/TableTavern/worlds/<id>/`（refactor-outcome.json／worldbook.json／mechanism-log.jsonl 可機械核對）。**供應商只認 claude／codex**——grok／agy 無隔離旗標（見環境陷阱），切過去測出來的盤點不算數。
- ~~T1 兽人的洞穴~~：**全過收檔**（2026-08-12，見 Current state）。
- **T2 NorthHall（23 條八角色）｜擱置**：①每人「速览段＋人物設定＋性格」跨條合成一張完整卡不碎裂 ②三條「剧情-」觸發歸機制線（absorb 或 group mechanism）③「格式增强Plus」整條淘汰 ④「美化状态栏」三路拆＝欄位綱要→STATE（emoji 標籤留值）、行動選項→GM 規則條目、容器紀律→drop①。
- **T3 Transfur（16 條盲測定案）｜擱置**：①目錄四條＋keyed 地區四條 carry 含元資料（keys 在）②核心設定歷史年表 carry 且該行附 reason（預掃衝突）③「格式」「COT」多路拆＝敘事行為指令→GM 規則、gametext 容器→drop①、擲骰／ASCII 地圖→unabsorbed 清單可見（欄位綱要→STATE 那半條已因 08-12 17:30 裁決降級為非判準）。
- **T4 通用｜今天跑**：①取消鍵中止在途＋Cmd-Q 無孤兒（refactor-dispatch P4–P6）②API 未設 balanced 退 GM（P8）③舊 refactor-outcome.json（無 meta/dropped 三欄）匯入照舊可套用 ④淘汰／未接管面板十語系文字正常。
  - ①-a **單發取消過**（2026-08-14 11:00，主線 ps＋jsonl 雙證）：survey 子程序（ppid＝app）取消後 1 秒內消失、無 ppid=1 孤兒、app 存活、prompt-cache.jsonl 零新增＝沒跑完沒計費。
  - ①-b **並行取消：底層過、UI 不過**（2026-08-14 11:20，orc-cave）。底層＝兩支在途（76685／76687）同時消失、app 存活、兩支皆無用量記錄。UI＝取消後彈出「**重構完成**」面板（拆出 6 角色・介面搬進 app・重建 8 條），跑到 1/3 按取消卻拿到可直接「全部套用」的半成品。根因 [WorldEditor.tsx:610](../../src/views/WorldEditor.tsx)：`runRefactorCalls` 取消只是提早 return，後面無條件 `assembleRefactorOutcome` 出面板；被中止的 pool 項在 :594 走 `refactor-aborted` sentinel 靜默略過，連 failedTitles 都不列，面板上看不出少東西。**拍板（2026-08-14 使用者）：保留面板、加註「這是取消後的部分產出」，並把主按鈕從「全部套用」換成「不要」**（僅取消造成部分產出時；不做預設不勾／未完成清單）。同輪實證這個洞為何危險：被殺的呼叫裡含機制接管，`剧情时间线跳跃补全指令` 因此退回 kind=setting 照搬——**缺件不缺畫面**，成品看起來與完整跑完幾乎無異。
  - ①-b **已修（2026-08-14，主線直寫，未 commit）**：`refactorCancelled` 狀態（run 起始清零、組裝面板時記錄 `refactorCancelRef`）→ 標題改 `refactorResultCancelledTitle`、加紅字 `refactorCancelledNotice`、主按鈕（`ai-gen-submit`）從「全部套用」換到「不要」。十語系各 +2 鍵。自驗：tsc 0／vitest 130／check:i18n 十語系 OK／build ✓。**待實機看畫面**。
  - ①-c **Cmd-Q 無孤兒過**（2026-08-14 11:34）：survey 子程序 84579（父＝app 84134）在途時 Cmd-Q，子程序與 app／tauri dev／vite 全數消失、零 `父1` 殘留、jsonl 無新增。（11:31 前一次不算數：當時在途 0 支，沒走到 `kill_all_children`。）
  - ③ **舊產物相容過**（2026-08-14，使用者實測）：`worlds/01KZQ1G6Z6XCZEDCPX5ZGMFX71/refactor-outcome.json`（08-11 08:23，無 meta／無 dropped／無 audit）匯入 → 同桌與新桌都正常展開套用，角色與條目齊全、`陣營推進日程` 仍標「App 接管中」。新桌不改桌名＝預期行為。
  - ④ **已過**（2026-08-14 實看）：手工測試產物（淘汰 4／未接管 2／稽核 3）匯入後，面板骨架十語系正常切換——英、俄逐項看過（Dropped N items／Unadopted mechanisms／Audit／Missed content｜ОТБРОШЕНО／Непринятые механики／Аудит／Пропущенный контент），無爆版無漏翻；未翻的只有產物資料本文（title／note／detail），那是卡片內容不是 i18n 範圍。**惟新重構按鈕的淘汰機制若改寫，這幾塊字要重驗**（使用者拍板：屆時再看，不擋今天）。
  - ② **單元測試綠、實機延後**（2026-08-14 拍板）。**CLI 模式測不到這條**：[transport.rs:1767](../../src-tauri/src/transport.rs) `refactor_expand_tier` 只在 `transport_kind == "api"` 且 balanced 模型解析失敗時退 GM，CLI 一律 balanced。要實機得切 API 模式＋清空 balanced 模型跑一次（API 模式不生子程序，證據看 jsonl 的 lane 欄），成本比 CLI 訂閱高一個量級。現況把關＝三分支單元測試（transport.rs:2855）＋五處呼叫（absorb／group／person／statusbar／interface）接線主線逐處核過。哪天真用 API 模式時順手看一眼 lane 即可。
  - 存檔產物三份的 dropped／unabsorbed／audit **都是空陣列**（兽人的洞穴 ×2、西幻魔法世界模拟器 ×1），驗這兩個面板一律要自製產物走「匯入重構卡」。

## Next action
1. **T4 於 2026-08-14 收工**：①取消／Cmd-Q 過（UI 半成品洞已修，待實機看畫面）、③舊產物相容過、④十語系過（新按鈕改淘汰機制後重驗）、②單元測試綠實機延後。refactor-dispatch 的 P4–P6 隨 ① 一起綠，P8 同 ② 延後。
2. 取消面板修正（`refactorCancelled`）**尚未 commit**；實機看新畫面要等下次真跑（survey 完成後中途取消才會出現）。
3. 本案結案仍等 T1–T3——重構按鈕做完（refactor-mode-split 落地＋介面接管收尾）後補測。
4. 延後項：grok／agy 供應商隔離（補旗標＋實測）、環境殘留 ANTHROPIC_* 的本機清理（提示詞已交使用者）、lib.rs:600 `[survey-persons]` 診斷 eprintln 待刪。

## Constraints
- 新格式規格放 survey user 訊息端；survey／expand 共用 system 逐位元組相同紅線零觸碰（既有測試把關，span 標記在 context 內、各階段共用不破相等）。
- knownFields 單一權威＝小抄 FIELDS（取代鏈上累積）；並行上限 4 不上調。
- 不在封閉清單或拿不準→一律 carry；作者設計內容永不 drop。
- 驗收（主線）：orc-cave 總時長 <5 分＋照搬 byte 不變＋入侵劇情線接管可跑＋稽核綠；NorthHall 八角色一人一卡不碎裂；Transfur 16 條盲測項；過後補驗 refactor-dispatch P4–P6/P8。
