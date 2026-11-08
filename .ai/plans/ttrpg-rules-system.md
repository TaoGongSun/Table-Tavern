# 跑團規則系統：規則書引入＋擲骰＋角色紙（規則中立引擎，零內建內容）

本檔存放 [ttrpg-rules-system](../handoffs/ttrpg-rules-system.md) 的規格細節（拍板結論、分包、驗收等），由任務檔的 Summary 連回。

## 拍板結論
1. **產品形態＝零內建**：v1 唯一新產出是「去哪拿規則世界書」指南文案（D&D 指 SRD 系開放來源如 Open5e，其餘指社群整理檔）；現成 ST 規則世界書今天就匯得進。AI 整理精靈（貼生文字→AI 拆條目＋配關鍵字→先報價、點頭才燒額度）列 v2 候補，需求再起才做。
2. **角色紙拆兩層**：初始宣告（職業／屬性／技能／裝備）放角色卡編輯頁「角色紙」區塊，預設唯讀＋解鎖鈕（防誤觸；單機明文檔防不了作弊、也不必防）；動態數值（HP／法術位／狀態效果）放狀態欄角色區——開桌時由卡種入，遊戲中 GM 協定更新、逐則快照可回滾。不做獨立角色紙頁。v1 過渡：初始紙寫卡片筆記欄即可玩。
3. **規則注入**：沿用關鍵字觸發；規則條目維持「只餵 GM」（餵 1 個 AI 而非 1+N 個，最大省法）。戰鬥高頻詞（攻擊／豁免）期間近常駐＝行為正確、帳要實測：v1 開工先拿一本現成 D&D 世界書實測一場戰鬥、量每回合增量，數字醜再拍「注入上限」（超額按 order 取前）。
4. **擲骰＝骰池預餵**：app 每回合真隨機擲好一排（按骰型分列）塞 GM 提示詞，指示照序取用、敘事標骰號（例「(d20#2=19)」）；同池顯示於畫面供玩家對帳，用罄才喊補。宣告歸角色、裁決歸 GM：角色 AI 無規則書、只演意圖，數字全由 GM 訊息處理（無卡雜兵 GM 代演代擲；獨立合批、依賴切段）。玩家骰鈕：僅玩家行動回合可按，面板＝regex 從最近 GM 訊息抓 `XdY±Z` 成一鍵選項＋自訂列（骰型／顆數／加值），結果以公開訊息進對話史。開關：世界書匯入偵測骰子記號→提示「要開擲骰嗎」，桌設定留手動開關。不做：模型自報（偏骰實證——模型會擲劇情要的數字）；兩段式喊骰協定降為 v2「嚴格模式」（無快取端點該回合近 2× 且等待×2）。`{{roll}}` 巨集維持原樣（st 第二項拍板），將來掛同一亂數引擎。
5. **邊界**：狀態欄任務管容器（自訂欄位 schema、協定格式、面板角色分頁、快照回滾），本任務管跑團內容（初始值來源＝卡、種入時機＝開桌、唯讀鎖、骰子、注入帳）。

玩家骰鈕 v1 限單人桌；房間 GM 照用骰池與骰號，房間骰鈕（含手機網頁端）列 v2。〔作者裁決 2026-11-08〕

## 分期
- **v1（不依賴狀態欄任何一期；重估大案、核心 900–950 行〔模型判斷·未裁決〕）**：指南文案＋骰池＋玩家骰鈕＋骰子記號偵測開關（字串 ×10 語系）＋注入實測回報。做完 D&D 即可玩：HP 由 GM 敘事口頭記帳（同「兽人的洞穴」純提示詞玩法）。
- **v2（等 [table-status-panel](../handoffs/table-status-panel.md) 角色格啟用後細拍；原寫「等狀態欄二期」，二期已以 state-values-mvu 結案）**：卡上角色紙結構化區塊＋唯讀鎖＋開桌種入；嚴格模式（兩段式喊骰）；AI 整理精靈；房間玩家骰鈕（含手機網頁端）〔作者裁決 2026-11-08〕。

## 驗收（v1）
- 匯入現成 D&D 規則世界書開桌：GM 敘事帶骰號且與畫面骰池對得上；模型亂取骰時遊戲照常（容錯同狀態欄，絕無報錯中斷）。
- 單人桌玩家行動回合骰鈕出現；GM 喊骰被 regex 抓成一鍵選項；擲果以公開訊息進對話史、其他 AI 下回合讀得到。
- 擲骰未開、目前非玩家時段或其他條件不符時，骰鈕停用並說明原因；桌設定可手動開關。〔模型判斷·未裁決〕
- 注入實測數字寫回本檔（每回合增量），據以拍板注入上限做不做。
- cargo test 蓋骰池生成、regex 解析、偵測開關。

## v1 技術方案

v1 維持骰池、單人骰鈕、指南與本節常識審修正；房間骰鈕、角色紙結構化、嚴格模式與整理精靈均留 v2。〔作者裁決 2026-11-08〕以下工程選擇與主線採納的審查修正均為〔模型判斷·未裁決〕，不冒標作者裁決。

### 現況與共用入口
- `evaluator.rs` 是算術引擎；既有 `FieldKind::Roll` 在 `mechanism/derive.rs` 由結算路徑重擲，時序不同，不改它。`st_macros/library.rs::roll_dice` 已有骰式，`{{roll}}` 維持原樣。單人 `solo_relay.rs` 與房間 `room.rs::TauriEngine::narrate` 均進 `gm.rs::gm_narrate`，在這個共用入口、`TurnGuard::begin` 後建立本輪池，GM 指示供 API 與 lane 共用，不在兩 host 各擲一次。〔模型判斷·未裁決〕
- `TurnRecord` 是記憶體狀態，適合保存本次呼叫的池；`prepare_turn_append` 是正常／半截／代落／重送正文的共用落檔判斷。房間 `gen.part.end` 與 `room/seed.rs` 是白名單投影，並不自動透傳整個 GM 事件，仍需接一個可選池欄位。〔模型判斷·未裁決〕

### 骰池表示、持久化與重試（S1／S2）
- Rust domain 用 `src-tauri/src/dice.rs` 集中亂數、固定池格式、骰式解析與結果文字；沿用 `getrandom::fill`＋拒絕取樣，亂數失敗明確回錯、不送模型，不退回模型自報或時間種子。每次 GM 旁白一池，NPC 接話不配；固定 32 顆：d4×2、d6×12、d8×2、d10×2、d12×2、d20×8、d100×4。〔模型判斷·未裁決〕
- **S2 採固定格式字串**：`TranscriptEvent.dice_pool: Option<String>`，Rust 唯一格式化為 ASCII 多行 `d4:[1,3]`、`d6:[4,2,...]`（實際只含完整數字，不含省略號）、固定骰型順序；完整池上限 512 UTF-8 bytes。序號由每列位置導出，池身分沿用正文的 event ID／turn_key，不另存冗餘 turn_id。serde default／缺欄相容舊檔，不新增池 system 事件、不預寫池。〔模型判斷·未裁決〕
- 字串的取捨：party 只需驗可選字串的長度、沿既有文字方式傳遞；桌面與手機共用 `src/features/dice/` 內的純 TS 固定格式解析／對帳模組，不各寫一份 parser。UI 解析時核對合法骰型、唯一列、顆數與面值範圍，異常只顯示無法讀取提示、不當 HTML 或公式執行。多一個很小的讀取 parser，換掉跨 Rust／party 的結構與逐層驗證；Rust 的 `XdY±Z` parser 處理另一種輸入，不與池格式混用。〔模型判斷·未裁決〕
- **S1 採單點掛載**：建立池後在短提交內存 `TurnRecord.dice_pool`；同一份字串放本輪 GM 指示。擴充 `TurnAppend::New`，僅 `PART_MAIN` 由 `prepare_turn_append` 回傳紀錄內的池，追加處覆蓋掛上權威值，非 main 為 None；`Existing` 回原事件，不覆寫、不重擲。正常／中止半截／待落代寫／落檔重送全走同一點，不替 `GmNarration`、`Piece`、房間 Narration、`PendingMain`、`main_part`、`TurnGuard::abort` 逐層加池欄位。〔模型判斷·未裁決〕
- 同一次呼叫的供應商重試沿用 `TurnRecord` 那池；玩家以新 ID 重試／重送配新池，不加 retry anchor。無正文落成就丟池；保留中止半截正文則帶原池；已落旁白重開 app／重連讀原池，落檔前退出無保存保證。收回／復原／分岔沿用完整事件搬移，不為池改 floor 分組。事件重建核對清單包含 clone、匯出匯入、`vn_reroll` 使用的 `replace_transcript_text`：只換正文／raw，不丟可選池欄位。〔模型判斷·未裁決〕

### 事件歷史、CLI session 與容量
- **事件歷史不帶池；CLI 線累積是已知限制。** 池 metadata 不加入 event.text／raw，所以 API 重組歷史、角色提示詞、世界書觸發與摘要不另帶全量舊池；正文已有的骰號照常保留。GM lane 的 instruction 進 `gm_lane_turn.tail`，Claude／Grok／codex 等 GM session 回合後不抹，舊池與指示仍留在 session，不能聲稱「lane 歷史無池」。〔模型判斷·未裁決〕
- 將當輪骰池及其指示的保留量併入既有 `TailStale` 計帳輸入，與尾段世界書合成一次計帳，**每輪最多加一份**，無世界書但有池也加。只改計帳來源，不為計帳再送一次池；GM 凍結 system 帶啟用狀態，關閉時由既有 system-changed 重開清掉舊池。依原門檻「8 份，或至少 4 份且達固定容量 15%」在下一輪整線重開、從不帶池 metadata 的事件歷史重建，清掉 session 舊池。tokens／bytes 兩種單位皆計；維持既有失敗／中止丟線與計帳規則，舊計數未知依原路徑重開。〔模型判斷·未裁決〕
- `scene_budget/measure.rs` 同步估當輪池與指示，唯讀用固定占位，不擲真骰；CLI 實測另列 session 累積保留量與重開點，不能只量新一輪訊息。lane 測試須檢查實際送入 session 的尾段、連續兩輪保留、世界書＋池只計一次、無世界書池仍計帳，以及門檻重開後不重建舊池。〔模型判斷·未裁決〕

### 骰池輸入成本與 GM 指示
- 池採 `dY:[V,V,...]` 緊湊分列、不重印每顆標籤；依 `budget_tokens` 的逐字權重，84 顆本體約 168–228 估算 token，32 顆約 80–100。按實作繁中指示與區塊標記離線重算，單人含 32 顆約 263–283、房間 217–237；84 顆則為 351–411／305–365。另有凍結 system 的穩定開關一句，不反覆保存成新尾段；這不是供應商 tokenizer 或帳單，CLI session 累積成本另列。〔模型判斷·未裁決〕
- 共用指示：「本輪骰池：各骰型由左至右取用，每顆限用一次；引用格式固定為(d20#2=19)，多顆逐一標號並列加值與合計。用罄或缺骰型就明說待補，不得自報、挑號或重用。」單人追加：「玩家公開擲果直接採用。要玩家擲骰時，一律寫出完整骰式，例如1d20+3。引用玩家擲果只寫數字，不加骰號括號；骰號只用於本輪池。」房間改追加：「玩家不能擲骰，所有骰由你從池取用。」兩者互斥，由 host 明確傳入單人／房間模式，不以玩家名冊是否為空推斷；房間不邀真人按骰鈕；優劣勢仍由 GM 取兩顆並說明取高／低。骨架用既有繁中／英文，輸出依玩家語言。〔模型判斷·未裁決〕
- 骰號標準輸出為 **`(d20#2=19)`**，解析接受全形／半形括號及空白，骰型＋型內 1 起算序號＋原始面值；加值／合計寫在括號外，不混成面值。用罄採正文明說、下次 GM 呼叫配新池，不自動補叫模型；單人也可先自訂補擲，房間仍由 GM 處理。不依世界書掃描動態選型，避免提及某骰型就被誤判成整套規則；大量戰鬥可能需補池，留實測調整比例。〔模型判斷·未裁決〕

### app 對帳與房間顯示
- 旁白落成後，共用 TS 模組按正文順序抓 `[（(]\s*d\s*([0-9]+)\s*#\s*([0-9]+)\s*=\s*([0-9]+)\s*[）)]`，只對該則旁白的池核對，raw 不參與。引用到的格標「已用」；面值不符、同型未從 #1 連續取用／跳號、重複用同顆均標紅並顯示文字原因。不存在的骰型／序號顯示紅色引用提示，不能拿其他格代替；不改骰面、不阻斷故事。只驗面值與使用順序，不驗加值、命中、傷害或規則裁決，仍不等於 v2 嚴格模式。〔模型判斷·未裁決〕
- 桌面旁白內、房間桌面與手機故事列共用同一對帳語意；有池即顯示收合池列，正文未抓到骰號時加「未找到骰號」，不隱藏整列；解析同時接受全形／半形括號與空白。沒有池的舊事件不臆造結果；正文編輯／復原／重連後重算「已用」與警告，不保存衍生的 used 狀態。紅色同時附文字／圖示，不只靠顏色。〔模型判斷·未裁決〕
- relay 的 `PartClose` 從**已落檔回傳的 `TranscriptEvent`** 取池，傳至 `gen.part.end.dicePool?`，不經 `Piece` 加欄位。party validate 限長、reduce／StoryPart 原樣保留；`room/seed.rs` 從旁白事件補重連 seed；兩端故事 renderer 都解析同一字串。舊段缺欄照舊、手機只看池，不新增 frame 或房間骰鈕。〔模型判斷·未裁決〕

### 單人骰鈕、擲後雙軌與鎖內順序
- 聊天桌 composer 旁骰鈕條件不符時**停用並提示原因，不整顆隱藏**：未開擲骰、正在擲骰／生成、目前不輪到玩家、單人接力未結束、唯讀／重播或房間模式；非聊天遊玩畫面不新增骰鈕。沒有明確點名而在等輸入也算玩家時段；展開最近 GM 候選與自訂顆數／骰型／加值，不要求先有規則書；是否輪到玩家只由 UI 掃點名停用，Rust 僅擋仍進行中的接力，不重複掃 GmCall。〔模型判斷·未裁決〕
- Rust 提供最近 GM 顯示正文的候選，不重掃 raw／角色台詞／整段歷史；regex 為 `(?i)([0-9]*)d([0-9]+)(?:\s*([+-])\s*([0-9]+))?`，缺顆數為 1，再檢查 ASCII 字母／數字／底線及相連運算邊界，不把 `2d20kh1`、`1d6+1d4` 或超限骰式的一截當完整式；中文相鄰可抓。最多掃 32,768 字元，去重後最多 8 個候選。顆數 1–100、面數 2–1000、整數加值 −9999–9999，超限回錯、不截小；自訂列只留 min／max，範圍驗證統一由 Rust 負責。v1 不支援爆骰、保留最高、Fate 記號與複合式。〔模型判斷·未裁決〕
- 擲骰指令只接受骰式與預期幕號，不收客戶端骰面；在**同一個 `with_commit`** 依序 `refuse_during_turn`（先阻正在生成並代落待落正文）→ 比對 `current_scene` 與傳入幕號 → 查有無進行中的單人接力 → 驗開關／單人聊天桌／可寫條件、解析與擲骰 → 以 tx 內追加寫公開擲果。既有過期幕檢查只套 Player，system 擲果必須自行重驗；不可只在 UI／鎖外先查，檢查與追加不可拆成兩次獨立提交。〔模型判斷·未裁決〕
- 接力檢查使用 `solo_relay` 的進行中登記，涵蓋 GM／NPC 切段與回合已結束但接力仍在的空窗；讀完登記就放短 guard，不跨追加持 signals mutex。擲骰中停用按鈕與送出，IPC 不重送、失敗不自動重擲；拿不到成功收據就重讀逐字稿確認，不自行補擲。拿掉 request_id 與去重掃描。〔模型判斷·未裁決〕
- **雙軌**：擲果確實落成後，若輸入框空白，沿用現在空白送出的同一接力模式，自動請 GM 接著敘事，不另開模式；若已有草稿，先顯示擲果並完整保留草稿，等玩家按送出再走。controller 串接既有 GM 入口，不把擲果再偽造為玩家句；接續前核對桌／幕，若擲骰等待期間出現草稿就改等送出，不自動送玩家新文字。GM 續寫失敗時保留既有擲果，只重試敘事，不重擲。〔模型判斷·未裁決〕

### 公開擲果事件
- 擲果用 `kind=system`＋無資料的單位 marker `EventMarker::DiceRoll`、`gm_only=false`；本文由 Rust 寫完整骰式、逐顆面值、加值、合計，例如「沃恩 2d6+3: [4, 2] +3 = 9」；沒有玩家卡或卡名為空時省略名字，不寫固定語言的玩家稱呼；「擲」由 dice_roll 標頭依語系組。不再保存無讀者的 faces／total 結構、不放 request_id；已落檔本文是對話中結果來源，不解析它回填規則數值。〔模型判斷·未裁決〕
- 擲果不走 `append_player_event`、不改 owed 玩家宣告狀態；收回時自成一樓，復原原文不重擲。marker／共享標頭字典／event text／匯出／floor 一起接；這項 floor 調整只針對玩家擲果，與旁白附池自然進退分開。公開 system 本文沿既有 API GM／角色歷史與 `lane_event_line` 進下一次 AI 請求，空白自動接話也會先讀到它。〔模型判斷·未裁決〕

### 開關、匯入提示與玩家指南
- **A 包包含** serde default 的 `WorldState.dice_enabled: bool`（舊桌 false）及只更新此欄的專用指令，能獨立驗開／關；非聊天桌不啟用。從 false 成功變 true 時追加一句公開 system 說明，同值重存不重複；單人說「已開啟擲骰：GM 敘事會標骰號，對話框旁可用骰鈕自己擲。」房間改說「已開啟擲骰：所有骰由 GM 從骰池取用，敘事會標骰號。」開啟通知不自動叫模型。B 包接桌設定 UI；骰鈕與設定共用 tableState.diceEnabled，hydrate／refresh 同步開關，onDiceChanged 一併刷新 tableState 與逐字稿，不在兩個骰子元件另讀整份 read_state。〔模型判斷·未裁決〕
- **B 包匯入提示**：此次實際匯入的啟用條目內容／keys 命中合法骰式，且目前未開時，提示「要開擲骰嗎？開啟後，GM 敘事會標骰號，對話框旁會出現骰鈕，讓你自己擲。」房間用「開啟後，GM 會從骰池取用所有骰，敘事會標骰號；房間玩家目前沒有骰鈕。」不自動開、不因書名猜測；直接世界書與角色卡隨身世界書皆涵蓋。確認後呼叫 A 的同一開關指令與通知；拒絕維持關閉。撤銷匯入不撤掉後來手動啟用的設定。〔模型判斷·未裁決〕
- 世界書匯入入口與桌設定只放指南連結／小彈窗，不堆整段文案。第一段：「開擲骰就能跑團，GM 用常識裁決；想更嚴謹再匯入規則世界書。角色數值先寫在卡片筆記。」第二段介紹下列來源：玩家自行取書，PDF／一般 SRD 資料須先整理成 ST 世界書；不寫「不代購」或 HP 開發過渡說明，不內建規則內容。房間指南補一句「房間目前由 GM 擲骰。」〔模型判斷·未裁決〕
- app 清單一行只有「系統名＋一個官方連結」，選 D&D SRD 5.1、D&D SRD 5.2、Pathfinder 2e Remaster、Fate、Cypher CSRD、Year Zero Engine SRD、Dungeon World、Basic Fantasy 4e、Cairn、BRP、Cthulhu Eternal、Blades SRD、Ironsworn SRD；官方連結直接取本檔下方對應列的來源／授權下載入口，不列未開放的 CoC 或沒有統一授權的泛 PbtA。Fate、Year Zero、Ironsworn、Blades SRD 名稱旁加「骰鈕不支援此系統骰法」，其他需要桌上流程或特殊操作也不承諾自動裁決。底下一句「以上為開放授權；其他系統請自行確認」。完整 15 項授權／社群檔查核留本檔，不搬進 app。〔模型判斷·未裁決〕

### i18n、核心預算與分包
- 十語系 UI key：`diceEnabled`、`diceEnableDetectedTitle`、`diceEnableDetectedBody`、`diceEnableDetectedRoomBody`、`diceEnableConfirm`、`diceKeepDisabled`、`diceEnabledNotice`、`diceEnabledRoomNotice`、`diceButton`、`diceSuggestions`、`diceNoSuggestions`、`diceCustom`、`diceCount`、`diceSides`、`diceModifier`、`diceRoll`、`diceRolling`、`diceInvalidFormula`、`diceLimits`、`dicePoolTitle`、`dicePoolId`、`dicePoolUnavailable`、`diceUsed`、`diceFaceMismatch`、`diceOutOfOrder`、`diceRepeated`、`diceUnknownReference`、`diceDisabledOff`、`diceDisabledBusy`、`diceDisabledTurn`、`diceDisabledRelay`、`diceDisabledReadonly`、`diceDisabledRoom`、`diceResult`、`diceRetryNewPool`、`rulesGuideTitle`、`rulesGuideBody`、`rulesGuideSources`、`rulesGuideRoom`、`rulesGuideOpenNotice`、`rulesDiceUnsupported`；標頭字典補 `dice_roll`。錯誤用代碼／參數轉譯，GM 骨架只補繁中／英文。〔模型判斷·未裁決〕
- 核心重估 **900–950 行，大案（>300）**，不含測試、i18n 譯文與純型別連帶；測試量不超過核心兩倍。S1／S2 與簡化 marker 省下逐層搬運及去重，新增 app 對帳、lane 計帳、擲後雙軌與停用原因仍需程式，不能維持上一版行數。〔模型判斷·未裁決〕
- **A 包「GM 骰池與對帳」實作核心 359 行、測試 321 行**（新增非空白程式行，扣註解、純型別連帶與譯文；測試接線計入測試）：均勻亂數／32 顆池／固定字串、TurnRecord／prepare_turn_append 保存、單人／房間指示、TailStale／容量、dice_enabled 後端／通知、共用解析對帳與桌面／房間即時／seed／sync 顯示均已接上。A 可直接呼叫設定指令驗開／關，不依賴 B 的 UI。〔模型判斷·未裁決〕
- **B 包「單人骰鈕與指南」核心 557、測試 351 行**：候選／自訂／鎖內擲骰、公開事件與擲後雙軌，桌設定、兩路匯入提示、指南小彈窗及十語系；完整非同步守門與畫面接線納入核心計數。沿用空白送出模式，成功回傳的擲果立即顯示並重讀逐字稿；開關／匯入成功也重讀，系統說明立即出現。單人公開擲果進既有房間 seed／sync，僅補歷史顯示，不加房間骰鈕。注入量測另輪處理，不增加規則引擎或模型整理功能。〔模型判斷·未裁決〕

### 規則書取得與注入實測
- 授權清楚、可直接匯入的現成 D&D ST 書優先；目前未確認合格檔，依主線裁定取官方 SRD 5.1 CC BY 4.0 機械轉 ST 條目，只放 gitignored `TestCards/`，保留來源／授權／署名／章節／關鍵字、版本／下載日／雜湊；不混 5.2 或其他出版者，不使用模型、不把規則放 app。這是 SRD 基準，不冒稱現成社群書。來源與其他可用規則見下方完整清單。〔模型判斷·未裁決〕
- 先離線量固定規則書與至少 6 個 GM 戰鬥回合（雄性獸人沃恩、雷克；含攻擊／優劣勢／傷害／豁免／法術／結束）：用真正 `gm_scan`、`gm_messages`／`gm_lane_parts` 成對組裝，列有／無規則書差值、骰池指示增量、觸發 uid／order／條目數、字元／UTF-8 bytes／量測單位與估算 token；chars 線核對 GM-only 規則未流入。用隔離副本與固定亂數推掃描計時／黏著，不落正式 landing；CLI session 尾段累積及重開成本另量，不以 API 無歷史池推論 CLI。〔模型判斷·未裁決〕
- 真模型戰鬥另先報費用、作者確認才跑；本輪未呼叫模型、未下載／轉檔規則書、尚無每回合真實帳單。據實測再拍規則注入上限，不先加硬上限。〔模型判斷·未裁決〕

### 驗證與已知限制
- A：Rust 測面值／拒絕取樣邊界、固定格式及設定開／關；同次呼叫重試共池、新 ID 新池，`prepare_turn_append` 正常／Aborted／代落／Existing 同池、非 main 不帶池；事件搬移與 `vn_reroll::replace_transcript_text` 不丟欄位。前端驗已用／跳號／重號／錯面／越界、未引用仍顯示收合池列與未找到骰號提示、重載重算；房間即時與 seed 一致。lane 另驗 session 尾段累積、TailStale 單次計份與門檻重開，不能用 transcript 斷言代替。〔模型判斷·未裁決〕
- B：驗生成中／舊幕／接力切段空窗都拒絕擲果追加；公開擲果在下一次 API／lane 的 GM／角色側可見；空白自動續寫、有字不送草稿、落果失敗不續寫、GM 失敗不重擲；按鈕停用與原因、開啟通知只一則、直接書／隨卡書提示與房間特化文案。整案跑 `npm run verify` 與 test-harness 截圖，交主線親看。〔模型判斷·未裁決〕
- 已知限制：CLI session 在門檻重開前仍累積舊池；app 能核對已生成面值與順序，但模型提前看見整池，仍可能改敘事或做錯裁決，紅字不攔故事。規則硬裁決、房間玩家骰鈕、結構化角色紙、嚴格模式、整理精靈均不加進 v1。〔模型判斷·未裁決〕
- 主線已定重試 A（新 ID 新池）、補池 A（下次 GM 呼叫）、測試書合格現成優先／否則官方 SRD 機械轉檔；本節無新增待作者選項。最近完整 verify 在 party Node server `listen EPERM` 停於第 12/17 步（222 通過／5 失敗，第 1–11 步通過），不是全綠；本輪文件修改只做文件檢查，不冒稱功能或實測完成。〔模型判斷·未裁決〕

## 開放規則清單

查核基準日 2026-11-08；本表是指南與測試書選材建議，依所連官方文件判讀，均為〔模型判斷·未裁決〕。開放範圍以指定 SRD／版次為準；免費下載、程式庫 MIT、卡片作者允許轉載，都不能替未授權的規則文字、插圖或設定補授權。玩家可把符合授權的文字整理成自備 ST 世界書，分享時保留署名、授權、修改聲明及各條款要求；app 仍只給指南，不附書。〔模型判斷·未裁決〕

「未確認現成檔」表示本輪以規則名＋SillyTavern／lorebook／character card，在公開網頁、GitHub、Hugging Face 與可索引的 Chub 結果中未找到且核對通過，並非斷言不存在；沒有登入私群或付費取書。表中「可整理」是依授權範圍的判讀，不保證任一同名社群檔合法或可直接在本 app 執行。〔模型判斷·未裁決〕

| 規則／可用範圍 | 授權與條款 | 官方取得來源 | ST 社群世界書／卡片隨附書與授權查核 |
| --- | --- | --- | --- |
| D&D 5e SRD 5.1 | [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/legalcode) 或 [OGL 1.0a（官方 PDF 內）](https://media.dndbeyond.com/compendium-images/srd/5.1/SRD-OGL_V5.1.pdf)，建議用 CC 版；只含 SRD，不含整本 PHB／官方設定。〔模型判斷·未裁決〕 | [官方 SRD 頁](https://www.dndbeyond.com/srd)、[5.1 CC PDF](https://media.dndbeyond.com/compendium-images/srd/5.1/SRD_CC_v5.1.pdf)。 | 未確認授權清楚且可直接匯入的純 ST 5.1 規則書；[StatefulLore](https://github.com/cgstever/StatefulLore) 是程式擴充、[Multihog](https://github.com/MultihogAurelius/SillyTavern-MultihogDnDFramework/blob/main/docs/multihogDnDdoc.md) 含混合版次玩法，不能當成已核清的 SRD 世界書；前者 MIT 只足以確認程式授權。〔模型判斷·未裁決〕 |
| D&D SRD 5.2／5.2.1（2024 規則系） | [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/legalcode)；5.2 系不另提供 OGL 版本，仍只開放 SRD。〔模型判斷·未裁決〕 | [官方 SRD 頁及 5.2.1／舊 5.2.0 下載](https://www.dndbeyond.com/srd)、[5.2.0 CC PDF](https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.pdf)；選材固定版本，不與 5.1 混合。 | 未確認清楚標示 5.2／5.2.1 來源與署名的現成 ST 書／隨附書；不能把泛稱 D&D 5e 卡當成 5.2。〔模型判斷·未裁決〕 |
| Pathfinder 2e Remaster | [ORC 條款／AxE](https://paizo.com/orclicense)、[Paizo 授權說明](https://paizo.com/licenses)；只用 Licensed Material、保留 ORC notice，排除 Reserved Material；舊 OGL 內容不能自行改標 ORC。〔模型判斷·未裁決〕 | [Paizo Remaster 說明](https://paizo.com/blog/pathfinder-second-edition-remaster-project)、官方合作規則庫 [Archives of Nethys](https://2e.aonprd.com/Rules.aspx) 及其 [授權頁](https://2e.aonprd.com/Licenses.aspx)；AoN 同時有 Community Use／舊 OGL 內容，不能整站視為 ORC。 | 未確認合授權且純 PF2e Remaster 的現成 ST 書；[Multihog 文件](https://github.com/MultihogAurelius/SillyTavern-MultihogDnDFramework/blob/main/docs/multihogDnDdoc.md) 自述混用 3.5／Pathfinder 與 5e，不拿來替代 PF2e Remaster。〔模型判斷·未裁決〕 |
| Fate Core／Accelerated／Condensed SRD | [CC BY 3.0](https://creativecommons.org/licenses/by/3.0/legalcode)，另有 OGL 選項；用各 SRD 的指定署名，不概括套用到所有 Fate 設定書。〔模型判斷·未裁決〕 | [官方認可授權頁與原始 SRD ZIP](https://fate-srd.com/official-licensing-fate)；該頁要求製作衍生內容取官方下載稿，而非直接抓改排過的網站全文。 | 未確認授權清楚的現成 ST Fate 規則書／隨附書；[Fate Core ST 討論](https://www.reddit.com/r/SillyTavernAI/comments/1pmofni/fate_core_character_help/) 是使用者提示詞討論，無完整規則檔及可核對的規則授權。〔模型判斷·未裁決〕 |
| Cypher System CSRD | [CSOL 全文](https://csol.montecookgames.com/license/)（自訂授權，非 CC／ORC）：可用 CSRD，須相容性標示、獨立製作聲明，並遵守內容限制；不含 Numenera 等其他 MCG 書。〔模型判斷·未裁決〕 | [CSRD／授權下載](https://csol.montecookgames.com/license/)、[官方 FAQ](https://csol.montecookgames.com/faq/) 明確允許符合條件的 VTT 資料與線上角色產生器。 | 未確認現成 ST 世界書／隨附書；可整理符合 CSOL 的自備規則資料，不能稱為「無限制開放」或整本 Cypher 世界設定都能重製。〔模型判斷·未裁決〕 |
| Year Zero Engine SRD | [FTL 1.1（2026-03-31）](https://freeleaguepublishing.com/wp-content/uploads/2026/03/Year-Zero-Engine-License-Agreement-version-1.1.pdf)；限 SRD 與桌上／VTT 模組，排除電子遊戲與其他品牌內容，須授權／非背書聲明；包含生成式 AI 文字／美術的出售作品有標示要求，非全面禁止 AI。〔模型判斷·未裁決〕 | [Free League 現行授權入口](https://freeleaguepublishing.com/community-content/free-tabletop-licenses/)、[YZE SRD 1.0](https://freeleaguepublishing.com/wp-content/uploads/2023/11/YZE-Standard-Reference-Document.pdf)。本輪 PDF 直開 403，FTL 1.1 全文由官方 PDF 搜尋索引讀取；正式取書時仍需保留原檔。 | **找到作者宣告的 ST 世界書**：[GM-5 README](https://huggingface.co/sphiratrioth666/GM-5_Game_Mistress_Roleplaying_System/raw/main/README.md) 列 Year Zero 額外書與 worlds 匯入位置；repo 標 CC BY-NC 4.0，但未核清各書的 FTL 通知／內容來源，檔案樹本輪無法讀取，不能認定授權鏈完整或當成官方原文。〔模型判斷·未裁決〕 |
| PbtA 系（總稱） | **沒有統一的 PbtA 文字授權**；[創始者說明](https://lumpley.games/2023/11/22/what-is-pbta/) 是受啟發遊戲的標籤，不能據此重製 Apocalypse World 或任一 PbtA 商業書；應選下一列等有明確授權的作品。〔模型判斷·未裁決〕 | 依各作品官方 SRD／授權，沒有一份涵蓋全系的免費規則書。 | **找到泛 PbtA ST 書宣告**：[GM-5 README](https://huggingface.co/sphiratrioth666/GM-5_Game_Mistress_Roleplaying_System/raw/main/README.md)；作者 CC BY-NC 4.0 不能替其可能引用的別人文字補授權，未核清採哪套 PbtA 來源。〔模型判斷·未裁決〕 |
| Dungeon World（PbtA 具體開放例） | [CC BY 3.0](https://creativecommons.org/licenses/by/3.0/legalcode)；[作者 repo LICENSE](https://github.com/Sagelt/Dungeon-World/blob/master/LICENSE) 另標其中 SRD 5.1 內容為 CC BY 4.0，需保留混合來源署名；不把插圖／商標一併當開放。〔模型判斷·未裁決〕 | [作者官方介紹](https://www.dungeon-world.com/about/)、[作者原始文字 repo](https://github.com/Sagelt/Dungeon-World)。 | 未確認可獨立匯入且來源／授權完整的 ST 世界書；GM-5 的泛 PbtA 書不能直接認定就是 Dungeon World，Voyage 等 preset 的啟發說明也不等於完整規則書。〔模型判斷·未裁決〕 |
| Basic Fantasy RPG 4e（OSR） | [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/legalcode)，分享改編文字需遵守同授權；舊版與其他模組須逐份看，不把全站一律改標。〔模型判斷·未裁決〕 | [官方免費 PDF／原稿入口](https://www.basicfantasy.org/downloads.html)（本輪 403）；[作者發行書目](https://www.lulu.com/shop/chris-gonnerman-and-erik-wilson/basic-fantasy-rpg-core-rules-4thed-hardback/hardcover/product-rnedyj.html) 確認 4e 為 CC BY-SA；[r128 原書版權聲明的非官方文字鏡像](https://jmhimara.github.io/bfrpg/) 寫明 4.0，正式選材仍以下載稿為準。 | 未確認合授權的現成 ST 世界書／隨附書；可從官方 4e 文字整理，不需購買紙本。〔模型判斷·未裁決〕 |
| Cairn（OSR／NSR） | [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/legalcode)，依[官方衍生內容說明](https://cairnrpg.com/third-party/third-party-content/) 保留署名與同授權；第三方素材各自核對。〔模型判斷·未裁決〕 | [官方規則／下載](https://cairnrpg.com/)、[第一版 SRD](https://cairnrpg.com/first-edition/)；固定版次。 | 未確認合授權的現成 ST 規則書／隨附書；搜尋遇到同名記憶插件，不能誤認為 Cairn TRPG 規則包。〔模型判斷·未裁決〕 |
| Basic Roleplaying: Universal Game Engine（BRP，2023） | [ORC](https://www.chaosium.com/orc-license/)，只取公開 content document 中授權文字，遵守 credit 與商標保留；不要與 2020 舊版自訂 BRP OGL 混為一份條款。〔模型判斷·未裁決〕 | [Chaosium 官方 ORC 內容 PDF](https://www.chaosium.com/content/orclicense/BasicRoleplaying-ORC-Content-Document.pdf)、[官方設計遊戲說明](https://www.chaosium.com/getting-started-designing-games/)。 | 未確認授權清楚的現成 ST BRP 世界書／隨附書；可依法整理 BRP 百分骰規則，但這不是 CoC 7e 整書授權。〔模型判斷·未裁決〕 |
| Call of Cthulhu／CoC 7e（邊界，非開放 SRD 項） | 未確認全規則以 CC／ORC／OGL 開放；[Chaosium 使用與授權 Q&A](https://www.chaosium.com/fan-use-and-licensing-q-a/) 的粉絲／Miskatonic Repository 規範不是任意重製、跨站散布規則書的通用授權。〔模型判斷·未裁決〕 | [官方免費入門資源](https://www.chaosium.com/cthulhu-quickstart/) 可供閱讀／遊玩，不因免費就能整本轉發；要做開放克系指南，改選 BRP 或 Cthulhu Eternal。 | 未確認合授權的完整 CoC 7e ST 規則書／隨附書；不把克系角色卡、神話設定書或其他 app 的程式碼授權當成 CoC 規則授權。〔模型判斷·未裁決〕 |
| Cthulhu Eternal SRD（克系開放替代） | [OGL 1.0a（官方 SRD 內）](https://cthulhueternal.com/open-rulesets/)；保留授權全文與 Section 15／Open Content 指定，logo 另依相容性條款，不能混入 Chaosium 專有設定。〔模型判斷·未裁決〕 | [官方各時代免費 SRD](https://cthulhueternal.com/open-rulesets/)、[官方開放內容說明](https://cthulhueternal.com/news/world-war-1-srd-released-today/)。 | 未確認合授權的現成 ST 世界書／隨附書；[社群線上 SRD](https://github.com/stories-with-dice/cthulhu_eternal_srd) 是規則網站來源，非 ST JSON，不冒稱直接匯入檔。〔模型判斷·未裁決〕 |
| Blades in the Dark SRD／Forged in the Dark | [CC BY 3.0](https://creativecommons.org/licenses/by/3.0/legalcode) 與[官方署名／標示說明](https://bladesinthedark.com/licensing)；只含 SRD，不含 Duskwall／Shattered Isles 設定、NPC、美術與地圖。〔模型判斷·未裁決〕 | [官方 SRD](https://bladesinthedark.com/basics)、授權頁連結的 SRD 文件。 | 未確認合授權的獨立 ST 世界書／隨附書；找到使用心得與其他 app，未找到足以核清授權的可匯入檔。〔模型判斷·未裁決〕 |
| Ironsworn／Starforged 開放參考內容 | [官方授權頁](https://tomkinpress.com/pages/licensing)：Ironsworn SRD／資產卡文字／oracle、Starforged Reference Guide 文字為 [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/legalcode)；完整規則書文字則為 [CC BY-NC-SA 4.0](https://creativecommons.org/licenses/by-nc-sa/4.0/legalcode)，不可合併標成同一授權。〔模型判斷·未裁決〕 | [官方授權／SRD 下載連結](https://tomkinpress.com/pages/licensing)、[官方免費下載](https://tomkinpress.com/collections/free-downloads)；下載入口以授權頁實際連結為準，不預設完整書等同 SRD。 | **找到 ST 整合**：[Ironsworn with AI](https://github.com/HijackHornet/SillyTavern-Ironsworn-with-AI) 依賴 Multihog／IronswornRoll、以 PDF databank 與 cartridge／生成 lorebook 組成，非單一通用 ST 書；[repo LICENSE](https://github.com/HijackHornet/SillyTavern-Ironsworn-with-AI/blob/main/LICENSE) 是 CC BY 4.0 並稱原作同授權，但其中完整書 PDF 與官方 NC-SA 範圍有衝突，不能整包當授權已核清。〔模型判斷·未裁決〕 |

本節完整表供開發者查核來源與授權；app 僅採「v1 技術方案」指定的系統名＋官方連結簡表與骰法支援標示，不搬入完整 15 項或未開放／泛稱項目。授權合格不等於 v1 能自動裁決：Fate 骰記號、YZE 推骰、Ironsworn oracle、Blades 取高等仍不承諾支援。D&D 注入量測依主線條件採官方 SRD 5.1 機械轉檔，本輪未下載／轉檔規則內容或呼叫模型。〔模型判斷·未裁決〕
