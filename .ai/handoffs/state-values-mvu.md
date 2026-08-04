# Handoff: state-values-mvu

## Current state
2026-08-04：包 1（標籤放寬）、包 2（機制格式核心）、包 3（`[initvar]` 匯入）、包 4（增量解析＋本地權威＋統一協定）完成，cargo test 262 綠、勇者實卡煙霧驗過。下一步＝包 5（注入策略＋分支切割，大）。

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
- 順手帶（interface-card-panel 交界）：`TranscriptEvent` 加 `raw` 欄位存剝殼前的模型原文，只在真的剝到東西時才存，舊檔沒這欄照樣讀。`gm_narrate` 回傳 `raw`、前端存進事件；角色台詞本來就沒剝殼，不需要。

## Verification
- 包 4：`cargo test` 262 passed（234→262，新增 28）；clippy 與 fmt 逐字持平（lib 9／lib test 10、既有 6 檔不符 rustfmt）。
- 包 4 勇者實卡煙霧（跑完即刪）：抽出 19 條規則（`World.Invasion` 0–100、`World.Roll100`／`Roll20` 判成骰值欄、`Heroes.*.HP` pair、`Heroes.*.Affection` 0–200、`Player.Level` 的「零阶-六阶」不填 min/max），`incremental=true`。餵一段含壞逗號的 `<UpdateVariable>`：HP delta -80 → 500/500 變 420/500；Invasion 給絕對值 77 被拒、本地帳留 1；Affection +500 夾到 200；模型改 Roll100 被拒、本地連擲 12 次得 11 個不同值；壞逗號後面的 Location 照樣套用成「晨港」；不存在的路徑記硬錯誤。正文只剩「亞瑟握緊了劍。」。
- 包 1–3：`cargo test` 234 passed（228→234，新增 6：初始樹形狀／只補不覆蓋／壞 YAML 不擋匯入／啟用中的同名條目略過／`parse_indented_fields` 分支標記／樹葉 `{{user}}` 代換）。主線自跑（codex 沙箱那 4 個 loopback 測試在主線是綠的）。
- `cargo clippy --all-targets` 與改動前逐字相同（lib 9、lib test 10）；`cargo fmt --check` 本包三個檔乾淨（codex 順手全檔 fmt 的 6 個無關檔案已 revert；repo 本來就有 6 個檔不符 rustfmt，維持原狀）。
- 一次性煙霧測試（跑完即刪）：兩張實卡走角色卡與世界書兩條路徑，樹形與獨立寫的 python 參考解析器逐項相同——勇者卡 5 個頂層／491 葉／94 分支／3 空容器，根源重塑 4／340／136／17，兩條路徑產出的樹 `assert_eq!` 相等。抽查值正確：`Player.Name = "{{user}}"`（字面）、`Heroes.亚瑟·晨光.HP = "500/500"`、`World.Title = "序章: 灵魂的降临"`（引號內的冒號沒被切）、`Player.Inventory = {}`（空分支）。

## Remaining
包 5–8 未開工，內容見 [tasks/state-values-mvu.md](../tasks/state-values-mvu.md) 分包段。順序建議：包 5 → 包 7 → 包 6／包 8。包 5／7 各自吃滿一次對話。
包 4 沒做、留給後面的：真桌實跑（本包只有煙霧測試，沒開 app 跑過真模型回合）；面板還沒有地方看 `mechanism-log.jsonl`（包 8）。

## Notes
- 樣本卡在 TestCards/（gitignore）。從 PNG 取卡片 JSON：讀 tEXt chunk 的 `chara`（base64 JSON），五張卡都有。
- 面板顯示仍是 `{{user}}` 字面（拍板 21 說顯示也該代換）：前端要拿到玩家名才改得動，併進包 5 的面板工作一起做。
- 勇者卡另有一組 `<CloverArchive>` 全回應 XML，是 ST 前端渲染用的另一套殼，本期不處理。
