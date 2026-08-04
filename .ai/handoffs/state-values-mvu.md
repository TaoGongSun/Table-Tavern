# Handoff: state-values-mvu

## Current state
2026-08-04：包 1（標籤放寬）、包 2（機制格式核心）、包 3（`[initvar]` 匯入）、包 4（增量解析＋本地權威＋統一協定）、包 5（注入策略＋分支切割）、包 7（觸發表＋固定型 EJS 解析）完成，cargo test 309 綠、勇者實卡提示詞煙霧驗過。下一步＝包 6（全量型跳動標記，小）或包 8（未收編帳本，小），兩包互不相干、順序隨意。

## Completed
- 包 1 標籤放寬（主線直寫）：`transport::find_state_tag` 前綴比對（`<StatusData>`、`<Status_block>` 皆認，`<combatStatus>` 不誤剝）；`<maintext>` 只拆殼留正文；`data::STATE_BAR_MARKERS` 補 ```` ```status ````。
- 包 2 機制格式核心（規格與驗收主線，實作外包 codex `gpt-5.6-terra`）：
  - `TableState` 拿掉佔位的 `characters`，換成 `tree: BTreeMap<String, StateNode>`（`StateNode` untagged：葉子＝字串、分支＝物件，JSON 形狀自然可讀）。逐則快照、`pop_transcript` 回滾、手動改欄位三條不變式自動沿用——樹存在同一份快照裡。
  - 欄位規則層 `Mechanism { version, rules }` 掛在 **WorldState**（`state.json`）而非 TableState：規則是這桌的設定、不隨對話變動，放進逐則快照等於每行 transcript 複製一份。`FieldRule` 七型別＋預留 `derived`，`FieldRule::for_kind` 給預設的更新方式／注入等級。
  - `extract_state_block` 改回傳 `(Vec<(Vec<String>, String)>, String)`：容錯縮排解析（tab 當 4 格、`- key:` 開子層、剝引號、沒冒號的純清單項跳過）。路徑長度 1 進 `table`（基礎三欄正規化照舊），>1 進 `tree`。鎮北王府因此變成真的樹（状态栏→用户列表→各人→各欄，四層），不再是平的 19 欄。
  - 注入：`gm_dynamic_block` 在平欄之後把樹以兩格縮排印出來（本期全送，相關分支切割歸包 5）。
  - 面板：狀態列下方樹狀折疊，第一層預設展開、深層收起；葉子點一下變輸入框，存回走新的 `set_state_path` command（空值＝刪葉子並剪掉空分支）。順手修好狀態列的 `background: var(--paper)`（這個變數整份 CSS 沒定義，一直是透明的，樹一長就看到底下文字透上來）→ 改 `--surface-2` 並加 `max-height: 45vh; overflow-y: auto`。
  - 匯出匯入：角色卡 `extensions.table_tavern = {version, rules, initial}`，rules 只帶 `branch == 卡名` 的、key 去掉分支前綴；匯入補回前綴並掛回 `state.tree.<卡名>`。壞資料略過不擋匯入。
  - 規範文件 `.ai/reference/MECHANISM-FORMAT.md`（欄位規則型別表、路徑寫法、存放位置、匯出格式、翻譯判準；協定聲明與觸發表待包 4／7 補）。
- 包 3 `[initvar]` 匯入（規格與驗收主線，實作外包 codex `gpt-5.6-terra`）：
  - 包 2 那段縮排解析器從 `extract_state_block` 抽成 `transport::parse_indented_fields(&str) -> Vec<(Vec<String>, Option<String>)>`；`None`＝這行只開一層分支（空值或 `{}`）。`extract_state_block` 濾掉 `None` 後行為逐字不變（既有測試原封通過）。
  - `import::import_initial_tree(root, world_id, book)`：認 comment 以 `[initvar]` 開頭**且已停用**（`enabled:false` 或 `disable:true`）的條目，內容解析成樹補進 `state.tree`。**只補不覆蓋**（`merge_state_node` 加 `overwrite` 參數，本 app 自家 extension 匯入仍傳 `true` 維持原行為）——玩到一半重匯不會把數值倒回初始。`{}` 保留成空分支；中途撞到既有葉子就跳過該筆。
  - 兩條匯入路徑都吃：`import_character`（卡片的 `character_book`）與 lib.rs `import_worldbook` command。
  - `{{user}}` 存字面，`render_state_tree` 注入前才過 `replace_st_macros` 換成這桌玩家名。

- 包 4 增量解析＋本地權威＋統一協定（規格與驗收主線，實作外包兩隻 general-purpose subagent `model: opus`）：
  - 新模組 `mechanism.rs`：`parse_updates`（`<UpdateVariable>` → 五種 op；剝 `<Analysis>`、認 `<JSONPatch>`、剝 ``` 圍欄、整段 JSON 失敗就退回逐個掃平衡 `{}` 各自 parse——模型漏逗號只掉那一筆）、`apply_updates`（依欄位規則收更新）、`reroll`（骰值本地重擲，亂數借既有 `ulid` 依賴）、`apply_block`（一則回覆套進這桌的唯一入口，`post_opening` 與 `gm_narrate` 共用）、`append_log`。
  - 規則比對：精確路徑優先，再找萬用段 `*`（取萬用段最少者），都沒有就依現值形狀推定（`n/n`→pair、數字→number、其餘→text）。拒收語意照拍板 5–7：replace 打數字欄一律拒、pair 的 replace 只認上限變動、骰值與 `_` 開頭欄位全拒；夾限與三類硬錯誤（路徑不存在／字串加減／刪不存在的鍵）各自記帳。
  - `extract_state_block` 改回傳 `StateBlock { fields, updates, display }`——`<UpdateVariable>` 原文不再丟掉。
  - 規則來源＝卡片的 `[mvu_update]` 規則表（`import_initial_tree` 併成 `import_mechanism`，初始樹＋規則一次掃完）：只收「路徑 ≥3 段且末段是 type／range／format」，ST 怪癖（`'${勇者姓名}'`→`*`、`${上装|下装}` 展開、`HP/SP/MP` 展開、段內 `.` 拆層）關在轉接層。
  - 收編（拍板 17）：`[initvar]`／`[mvu_update]`／含 `{{format_message_variable::` 的條目匯入即停用並記一筆 `absorbed`，面板既有的啟用開關就是「照原文送」開關。全量型卡不受影響。
  - 統一協定聲明（拍板 9）進 GM 凍結快照，只在 `mechanism.incremental` 的桌出現；zh／en 兩份。lane 那條走 `snapshot_patch` 補丁送達，舊線不必重開。
  - 拒收回饋（拍板 8）：`TableState.notes` 跟著逐則快照走，回合尾以「上一輪被系統擋下的更新」印出，收回上一句連回饋一起倒回。
  - 記帳落檔 `worlds/<id>/mechanism-log.jsonl`（ts／scene／kind／path／detail，四種 kind），寫檔失敗吞掉。包 8 的帳本直接讀它。
- 包 5 注入策略＋分支切割（規格與驗收主線，實作外包兩隻 general-purpose subagent `model: opus`，一隻後端一隻前端）：
  - 回合尾只送相關分支：桌級＋玩家＋在場角色。在場名單取平欄 `present`，`present` 空著就不裁（模型漏報一欄不該讓它整桌瞎掉）。
  - **手足規則（主線煙霧測試後補的）**：容器裡有一支綁到角色卡，同容器其他分支就一律當人看、不在場照裁。只在容器不是樹根時生效——頂層是 World／Player 這類桌級分支，套下去整桌被裁光。沒有這條的話勇者卡 15 個英雄只裁得掉有卡的那幾個。
  - 分支綁定：面板指認（`WorldState.branch_bindings`，卡 id → 路徑）優先，其次全樹同名比對（BFS、深度上限 3、取最淺）。指令 `set_branch_binding`（換綁自動移除同路徑舊綁定）／`branch_bindings`（含自動比對結果，`auto` 標記）。
  - 注入等級落地（拍板 18 依 Fable 修正改寫）：`turn` 每輪送、`rare` 不送、`snapshot`（長文字欄）平常輪不送——**變動落成 transcript 系統事件**，不放回合尾動態塊：動態塊每輪重組、不落歷史，API 那條傳輸路模型下一輪就看不到；transcript 兩條路都重播且吃快取。`gm_narrate` 回傳 `state_updates`，前端補一則 system 事件。凍結快照維持完全不含狀態文字。
  - 換幕全樹對齊：`WorldState.aligned_scene` != `current_scene` 就在該幕第一輪 GM 回合送整棵樹（排除 `rare`），標題明寫「以此為準」；模型真的收到才記，呼叫失敗下一輪再送。
  - 變動標記：`TableState.changes`（路徑 → `+5`／`-80`／`更新`）跟著逐則快照走，回合尾接在值後面（`HP：3920/4000（-80）`）。
  - 角色線只拿自己那支（含 `snapshot` 級、排除 `rare`），放**機密段**——chars 線全角色共用一條 session，放一般段下一個被點的角色就看得到別人的數值。API 單發那條路同樣帶。
  - 面板：每個分支 summary 一個指認下拉（未指認＋角色卡清單），`{{user}}` 只在顯示時換成玩家名（編輯框仍是原字面），玩家那支與其祖先預設展開。
- 順手帶（interface-card-panel 交界）：`TranscriptEvent` 加 `raw` 欄位存剝殼前的模型原文，只在真的剝到東西時才存，舊檔沒這欄照樣讀。`gm_narrate` 回傳 `raw`、前端存進事件；角色台詞本來就沒剝殼，不需要。

- 包 7 觸發表＋固定型 EJS 解析（schema 與規範文件主線直寫，實作外派兩包並行：辨識器 codex `gpt-5.6-terra`、求值與注入 general-purpose subagent `model: opus`）：
  - 觸發表 schema 進 `Mechanism.triggers`（不進逐則快照）：一組 `Trigger{id,title,mode,cases,preamble,scope,flag}`＝卡片一條腳本。`cases` **依序求值、第一個命中就停**，空 `when` 是兜底——來源 if／else-if 鏈（含巢狀優先級）攤平成一層，每筆帶上祖先條件，語意等價。命中文本存 `TableState.triggers`（id → 文本，跟著逐則快照走，收回上一句自動倒回）。
  - 條件三型：`Range`（含 exclusive 與 `default`，`"480/500"` 取現值）／`Contains`（任一子字串）／`Flag`。拍板 13 的第四型「計數器門檻」與數值區間判斷邏輯逐字相同，共用 `Range` 不另立型別。`min`／`max` 全空的 `Range` 兼作「這條路徑存在且是數字」。
  - `mode`：`range` 條件成立就持續注入；`once` 命中後把旗標路徑釘成 `"true"`（記一筆 `Absorbed`），下一輪條件自然不成立——事件演過就是演過。旗標路徑匯入時自動配一條 `read_only` 規則，模型改不動。
  - 新模組 `ejs.rs`（774 行，手寫掃描、零新依賴、**從不執行腳本**）：收 `getvar('stat_data.…')` 變數表（認 `Number(x)` 別名）、tokenize `<% %>`／`<%_ _%>`／`<%=`、遞迴解析 if 鏈、`<%= 變數 %>` 換成佔位 `{{state:<路徑>}}`。出現 `_.random`／`for (`／`.split(`／`Math.`／運算式內插／任一分支沒文本＝整條回 `None`。
  - **存在性守衛不能整層丟掉（主線煙霧後補的）**：卡片把整段腳本包在 `typeof X !== 'undefined' && X !== null` 裡，守衛剝掉後條件跟著蒸發，那個欄位還沒出現在樹上時兜底分支會無條件注入一段空值文本（灰烬侵蚀度／暴露度實測中招）。改成剝守衛時把變數收起來，每個 case 補一條存在性 `Range`。
  - 收編：content 含 `<%` 的條目匯入即停用（辨識成功與失敗都不送——原生 ST 是外掛執行腳本、模型本來看不到原文），認不出的記一筆新的 `RecordKind::Skipped` 進帳本，包 8 的開關直接讀它。
  - 注入：回合尾在「目前狀態」之後、拒收回饋之前插「## 當前情境（系統依狀態表判定的隱藏背景，不要在回覆裡複述本段）」，依 `mechanism.triggers` 順序印。**裁切沿用包 5**：trigger 的 `scope` 落在 `StateScope.hidden`（或其後代）就不印，換幕對齊時全印，全量桌整段不出現。
  - 連帶：`apply_block`／`append_opening`／`post_opening` 補 `user_name` 參數（觸發文本要代換 `{{user}}`）；`replace_st_macros` 開成 `pub(crate)`。

## Verification
- 包 7：`cargo test` 309 passed（272→309，新增 37）；clippy 逐字持平（lib 8／lib test 9）；`cargo fmt --check` 本包四個檔乾淨（codex 順手全檔 fmt 的 6 個無關檔案已 revert，維持 repo 原狀）；`npx tsc --noEmit` 與 `npm run build` 綠。
- 包 7 勇者實卡煙霧（跑完即刪，**沒有呼叫任何模型**）：30 條 EJS 抽出 **21 組觸發表**（13 條角色關係階段各 13 個 case、4 條環境氛圍、4 條國家事件），9 條認不出進帳本（擲隨機數的事件庫／報紙生成器、跑迴圈統計的決戰判定與通天塔啟示、算百分比的生理機能、沒有 `getvar` 來源的決心值監測、分支全空的區域自動加載）。4 個事件旗標各配到 `read_only` 規則。餵一則更新（亞瑟好感 5→60、欲望 0→20、侵略度 1→56、地點改聖葉國）：亞瑟關係階段換成「暧昧萌芽」；聖葉國一次性事件觸發、`Events.Ecclesia_Conflict_Triggered` 釘成 `"true"`、記一筆 `Absorbed`；不在場的 12 個英雄關係文本全裁掉，回合尾 1,884 字元；第二輪什麼都不改，事件文本消失、關係階段照留、回合尾 1,449 字元。
- 包 7 沒做：真桌實跑（要開 app 打真模型、花使用者額度，沒代做——包 4／5 也都欠這筆）；面板還沒有地方看觸發表與帳本（包 8）。
- 包 5：`cargo test` 272 passed（262→272，新增 10）；clippy 比改動前少一個警告（lib 8／lib test 9，基準 9／10）；`cargo fmt --check` 本包四個檔乾淨（subagent 兩次順手全檔 fmt 的 6 個無關檔案已 revert）；`npx tsc --noEmit` 與 `npm run build` 綠。
- 包 5 勇者實卡提示詞煙霧（跑完即刪，**沒有呼叫任何模型**）：匯入勇者卡→建兩張英雄卡＋一張玩家卡（分支靠同名自動比對）→餵一則 `<UpdateVariable>`（第一位英雄 HP -80、World.Location 改晨港碼頭）。結果：HP 4000/4000→3920/4000 帶 `（-80）` 標記；不在場的 14 個英雄分支全裁掉（有卡的 1 個＋手足 13 個）；回合尾 **3,127→530 字元**，換幕對齊輪 9,571 字元；角色線自己那支 506 字元、別人的名字沒漏進去；`World.Location` 沒出現在回合尾，改由 `snapshot_updates` 交給 transcript 系統事件；骰值欄 Roll100／Roll20 每輪本地重擲。
- 包 5 沒做：真桌實跑（要開 app 打真模型、花使用者額度，沒代做）；面板指認下拉只驗到型別與編譯，沒實機點過。
- 包 4：`cargo test` 262 passed（234→262，新增 28）；clippy 與 fmt 逐字持平（lib 9／lib test 10、既有 6 檔不符 rustfmt）。
- 包 4 勇者實卡煙霧（跑完即刪）：抽出 19 條規則（`World.Invasion` 0–100、`World.Roll100`／`Roll20` 判成骰值欄、`Heroes.*.HP` pair、`Heroes.*.Affection` 0–200、`Player.Level` 的「零阶-六阶」不填 min/max），`incremental=true`。餵一段含壞逗號的 `<UpdateVariable>`：HP delta -80 → 500/500 變 420/500；Invasion 給絕對值 77 被拒、本地帳留 1；Affection +500 夾到 200；模型改 Roll100 被拒、本地連擲 12 次得 11 個不同值；壞逗號後面的 Location 照樣套用成「晨港」；不存在的路徑記硬錯誤。正文只剩「亞瑟握緊了劍。」。
- 包 1–3：`cargo test` 234 passed（228→234，新增 6：初始樹形狀／只補不覆蓋／壞 YAML 不擋匯入／啟用中的同名條目略過／`parse_indented_fields` 分支標記／樹葉 `{{user}}` 代換）。主線自跑（codex 沙箱那 4 個 loopback 測試在主線是綠的）。
- `cargo clippy --all-targets` 與改動前逐字相同（lib 9、lib test 10）；`cargo fmt --check` 本包三個檔乾淨（codex 順手全檔 fmt 的 6 個無關檔案已 revert；repo 本來就有 6 個檔不符 rustfmt，維持原狀）。
- 一次性煙霧測試（跑完即刪）：兩張實卡走角色卡與世界書兩條路徑，樹形與獨立寫的 python 參考解析器逐項相同——勇者卡 5 個頂層／491 葉／94 分支／3 空容器，根源重塑 4／340／136／17，兩條路徑產出的樹 `assert_eq!` 相等。抽查值正確：`Player.Name = "{{user}}"`（字面）、`Heroes.亚瑟·晨光.HP = "500/500"`、`World.Title = "序章: 灵魂的降临"`（引號內的冒號沒被切）、`Player.Inventory = {}`（空分支）。

## Remaining
包 6（全量型跳動標記，小）與包 8（未收編帳本，小）未開工，內容見 [tasks/state-values-mvu.md](../tasks/state-values-mvu.md) 分包段；兩包互不相干、順序隨意，各自吃不滿一次對話。
包 4／5／7 沒做、留給後面的：真桌實跑（至今只有提示詞煙霧，沒開 app 跑過真模型回合）；面板還沒有地方看 `mechanism-log.jsonl` 與觸發表（包 8）。
已知取捨：`state.triggers` 存的是**求值時全部**的命中文本（勇者卡 16 組約 3KB），裁切在注入時才做——求值階段拿不到在場名單（`apply_block` 沒有角色卡），且若依在場裁切會讓逐則快照的內容隨在場變動，收回上一句就對不上。

## Notes
- 樣本卡在 TestCards/（gitignore）。從 PNG 取卡片 JSON：讀 tEXt chunk 的 `chara`（base64 JSON），五張卡都有。
- 面板顯示仍是 `{{user}}` 字面（拍板 21 說顯示也該代換）：前端要拿到玩家名才改得動，併進包 5 的面板工作一起做。
- 辨識器目前「任一分支沒文本就整條放棄」比規格更嚴（規格只要求「全部分支都沒文本才放棄」）：勇者卡 5 個可辨識樣本不受影響，若日後撞到「某一階段刻意留空」的卡再放寬。
- 勇者卡另有一組 `<CloverArchive>` 全回應 XML，是 ST 前端渲染用的另一套殼，本期不處理。
