# Handoff: state-values-mvu

## Current state
2026-08-04：包 1（標籤放寬）、包 2（機制格式核心）完成，cargo test 228 綠、面板實機驗過。下一步＝包 3（`[initvar]` 匯入，中）或包 4（增量解析＋本地權威，大且核心）。

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

## Verification
- `cargo test` 228 passed（222→228，新增 6 個：規則序列化往返＋舊檔預設空、`set_tree_value` 四情境、巢狀快照回滾、巢狀 YAML 解析、卡片匯出匯入往返、樹注入渲染）。主線自跑一次（codex 沙箱那 4 個 loopback 測試在主線是綠的）。
- `cargo clippy --all-targets` 與改動前逐字相同（lib 9、lib test 10），無新增；`cargo fmt --check` 本包四個檔乾淨（codex 順手全檔 fmt 的 6 個無關檔案已 revert）。
- 一次性煙霧測試（跑完即刪）：鎮北王府形狀輸出 → 四層樹、正文乾淨無裸露標籤；donass 形狀 → 仍是平欄。
- 面板實機驗收（tauri dev＋臨時假後端，驗完刪乾淨）：三層樹顯示正確、點葉子改值存得回去、按「收回上一句」整棵樹倒回前一則快照。

## Remaining
包 3–8 未開工，內容見 [tasks/state-values-mvu.md](../tasks/state-values-mvu.md) 分包段。順序建議：包 3（中）→ 包 4（大，核心）→ 包 5 → 包 7 → 包 6／包 8。包 4／5／7 各自吃滿一次對話。

## Notes
- 樣本卡在 TestCards/（gitignore）。從 PNG 取卡片 JSON：讀 tEXt/zTXt chunk 的 `chara`／`ccv3`（base64 JSON）。
- 包 3 的 `[initvar]` 是完整 YAML 文件（勇者卡 10,840 字元），本包這套容錯縮排解析器是為模型輸出寫的；真的擋不住再考慮引 YAML crate（目前零新依賴）。
- 勇者卡另有一組 `<CloverArchive>` 全回應 XML，是 ST 前端渲染用的另一套殼，本期不處理。
