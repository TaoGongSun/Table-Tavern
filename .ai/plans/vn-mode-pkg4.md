# vn-mode 包 4 方案：聖經、劇本分頁、劇透鎖、旗標、揭露

總綱 [vn-mode.md](vn-mode.md) §4（精靈 ②、聖經兩層注入、字數上限）、§5（秘密分層、劇透鎖、揭露）、§7（旗標）、§8（章號、結局表與保底結局）、§11 包 4 列。大案：方案與驗收都送 Sol。核心立案估 450；主線開工重估約 1000–1200（後端約 650、前端約 450），照流程上限只記不停工。本檔除標〔作者裁決〕者皆〔模型判斷·未裁決〕。

## 1 業界做法與我們的對應

| 業界 | 我們 | 差距 |
|---|---|---|
| 編劇室 series bible：主線、角色秘密與弧線、逐章節拍表（beat sheet）、伏筆埋收；工具 articy:draft、Arcweave、Notion 模板；先聖經→節拍→台詞 | 開桌時模型產結構化聖經（`vn.json`），GM 依常駐段＋當章包寫台詞 | 一致；聖經是模型即興寫的，品質靠重骰與玩家改 |
| ink `VAR`／Yarn Spinner `<<declare>>`／Ren'Py `default`：旗標先宣告型別與初值，劇本只讀寫 | 聖經旗標表開桌時寫成狀態樹初值＋機制規則，模型只以 `<UpdateVariable>` 報變動 | 一致；我們的判定是模型而非作者寫死的條件 |
| 平台 spoiler tag（點開才看）＋ VN 的 TIPS／劇情樹（隨進度逐條解鎖） | 劇透鎖遮罩＋解鎖；`revealed.<id>` 隨劇情逐條解 | 一致 |
| 節拍與章目標放在劇本工具、不給演員看全本 | 聖經只進 GM 線，角色線硬邊界看不到（既有 visibility） | 一致 |

## 2 定案設計

### 2.1 `vn.json`（`data/vn.rs`，`worlds/<id>/vn/vn.json`，原子寫入）

```
{ version: 1,
  spoiler_lock: bool,                          // 預設 true〔作者裁決 2026-11-01〕
  bible: null | {
    logline, style,                            // 主線一句、文風
    branching: "none"|"light"|"full",          // AI 生成預設 full
    chapter_max, chapter_reason,               // M 夾 3–30；缺或壞＝8
    chapters: [{ title, goal, beats, end_condition, plant[], payoff[], anchors[] }],
    secrets:  [{ id, holder: 角色id|null, title, text, reveal: {flag}|{chapter}|"never", hint }],
    flags:    [{ id, label, type: "bool"|"counter"|"route", initial, min?, max?, values?, trigger }],
    endings:  [{ id, title, when: [Condition] }]   // Condition＝data/state.rs 既有序列化
  },
  frozen: null | { scene, chapter, bible }     // 注入用凍結副本，見 2.4
}
```

- **建檔時機**：精靈 ② 一進來（呼叫聖經生成之前）就寫 `{spoiler_lock: ①的選擇, bible: null}`，所以生成中與失敗後按「繼續」都有鎖可套。讀不到檔的 VN 桌（舊桌）一律當成鎖住、沒有聖經。

- **驗證與正規化（一處，包 9 VN 卡匯入重用）**：id 一律 `[a-z0-9_]{1,32}`，重複或不合格的那筆丟掉；`holder` 對不到角色＝null；旗標 `route` 的 `values` 2–8 個、`initial` 須在內，`counter` 的 `initial` 夾進 min／max；結局 `when` 只認路徑 `flags.<表內id>` 與 `chapter`，條件不合格＝那條結局丟掉；結局表一定補一條 `default`（`when` 空）；`branching: none` 的非 default 結局自動補 `Range{path:"chapter",min:M}`。字數上限（總綱 §4）：常駐段 12,000 字、單章 4,000 字超出就截短，私設 2,000 字。
- 沒有聖經（`bible: null`）：可玩，結局表只有引擎自建 `default`（包 5 用）、章上限 8。

### 2.2 聖經生成：一支呼叫、三個入口

- 新提示詞 `transport/vn_bible_prompts.rs`：輸入 world.md、每角色公開與私設、GM-only 世界書條目，加上內容語言；輸出 `## BIBLE` 後面接一個 ```json 圍欄，欄位名用英文，內容用內容語言寫。genesis 的 expand 不動，聖經另外發一次呼叫。理由：genesis 新建、匯入角色卡建桌、整份重骰三個入口共用同一支呼叫與同一個解析器，不必另寫一套「讀卡生成聖經」。總綱 §4 原寫「expand 多一段 `## BIBLE`」，在此改掉。
- 解析失敗自動重試一次。首次生成再失敗，寫 `bible: null` 並在畫面提示（可再按重生成）；**重骰失敗則原聖經、私設、狀態一律不動**。
- **寫入契約**（Sol 方案審第 1 輪必改 1、3）：
  - 所有 `vn.json` 的讀改寫都走 `state_commit::with_commit` 的短提交鎖。鎖內先重讀最新檔，只改這次負責的欄位。涵蓋建檔、生成或重骰結果、劇本分頁存檔、劇透鎖切換、凍結寫回、補種。
  - 命令照常持有世界寫入許可（`world_write_permit_async`），用來跟刪桌、還原、更新清場這些獨占流程協調；GM 組裝本來就持有許可，直接沿用。
  - 寫入互斥由短提交鎖負責；模型呼叫一律放在短提交鎖外面（Sol 第 2 輪必改）。
  - 生成或重骰成功後，在同一把鎖內依序做：重驗未遊玩（只限重骰）→ 同步提交聖經或私設、狀態樹子樹、機制規則 → 換掉 `frozen`。
  - 整份重骰與角色秘密重骰共用同一個「套用聖經」函式。
- 呼叫走既有 GM 的模型與傳輸（同 genesis 命令的取法）。

### 2.3 旗標、揭露、章號落狀態樹

- 套用聖經時寫 `state.json`：
  - 樹：`flags.<id>`＝初值；`revealed.<秘密id>`＝`false`；`chapter`＝`1`（已有就不動）。
  - 規則：bool 是 `text`／`replace`，`allowed: ["true","false"]`；route 是 `text`／`replace`，`allowed`＝`values`；counter 是 `counter`／`delta` 加 min／max；`revealed.*` 是 `text`／`replace`，`allowed: ["true","false"]`；`chapter` 是 `read_only`（引擎自管，包 5 才會加一）。
  - 每條都 `inject: turn`。
- **`FieldRule` 加 `allowed: Vec<String>`**（空＝不限）。有 `allowed` 的欄位只收 `replace`：其他 op（insert／remove／move）一律拒收，新值不在清單內也拒收。兩種拒收都走既有回饋句，並附上「可用值：…」。機制格式文件同步補一行。
- **`allowed` 也要擋其他寫入入口**（Sol 第 1 輪必改 2）：
  - 合法值檢查抽成共用函式。
  - 面板手改 `set_state_path`（`commands/state.rs`）要過檢查，不合法就拒絕並回錯誤。
  - 觸發表 once 旗標直接寫入（`mechanism/ledger.rs`）也要過檢查，不合法時沿用既有的 `_once` 保底。
  - VN 桌的 `flags`／`revealed`／`chapter` 三支，不得整支替換、刪除、搬移，也不得新增表外的鍵；只能逐葉更新表內的 id。否則模型整支刪掉後，下一輪補種會把進度重置成初值。
- 只有建了聖經的 VN 桌才開 `mechanism.incremental = true`。查證：VN 桌不是接管桌，`BlockScope::Table` 本來就同時收圍欄（time／place／present）與 `<UpdateVariable>`；`updates` 旗標接上後，VN 導演指示與收尾句的「更新區塊」分支（包 1 已寫）自動生效。總綱 §7 寫的「抽成狀態協定由桌型決定」因此不需要新抽象，在此改掉。
- **補種**：每次 GM 回合套用更新前，把 `vn.json` 表內但樹上缺的 `flags`／`revealed`／`chapter` 路徑補成初值，只補不覆蓋。回溯到聖經建立前的快照（例如開場白那則）、或重骰後新增旗標，都能自癒。
- **重骰只限未遊玩的桌**（開場白以外沒有其他事件；比照既有「重構限未遊玩桌」）。
  - 整份重骰時，`flags`／`revealed` 子樹與對應規則整批換成新表，舊 id 刪除。
  - 已遊玩的桌，兩顆重骰鈕都改成提示「已有遊玩紀錄，要換聖經請開新桌」。
  - 理由：玩到一半換掉旗標表，舊快照的旗標會沒有規則，等於丟掉進度。
- 角色線不帶旗標與揭露：兩者都是桌級分支，照既有「角色只拿自己那支」本來就看不到。

### 2.4 注入（只進 GM 線）

- **常駐段**：主線、文風、`branching` 的語意句（總綱 §3）、每章一行標題與目標、秘密表（含揭露時機與 `never` 的暗示規則）、旗標表（id、說明、觸發情境、可用值）、結局表（id、標題、條件的白話），外加「揭露時以 `<UpdateVariable>` 把 `revealed.<id>` 設為 true」一句。
- **當章包**：本章節拍、結束條件、要埋與要收的伏筆、錨句。
- 兩段都接在 `vn_format_rules` 後面（`chat_assembly` 的 CLI 與 API 兩處）。zh／en 骨架照 `scaffold_en` 分流。
- **凍結**：注入一律讀 `frozen`。
  - 組裝時如果 `frozen` 不存在，或 `frozen.scene` 不等於目前幕號，就把當前的 `bible` 和樹上的 `chapter` 抄進 `frozen` 寫回。寫回走 2.2 寫入契約（短提交鎖內重讀），與劇本分頁存檔互斥。
  - 結果：幕中改聖經不動 system，**下一幕生效**；換幕本來就會重開 CLI session（`SceneChanged`），不會多斷一次快取。
  - 為何不照總綱：總綱 §4／§13 寫「下一章生效」，但章號要到包 5 才會加一，照寫的話改動永遠不生效。章結束必定換幕，以幕為鍵就涵蓋了章，在此改掉。
- `chapter` 與 `revealed` 的現值走回合尾既有動態塊（`inject: turn`）。

### 2.5 劇透鎖與遮罩（前端，君子鎖，總綱 §5）

- **元件 `SpoilerMask`**：顯示「劇透區：已鎖」，下面兩顆鈕：「不看就重骰」與「解鎖」。按解鎖要二次確認，寫明解鎖後會看到結局與秘密。解鎖後在原位置留「重新上鎖」。鎖的狀態存 `vn.json` 的 `spoiler_lock`。
- **套用範圍**：只限 VN 桌且鎖住時。
  - 角色編輯頁的私設欄（`CardEditor` 私設欄，玩家卡除外）。
  - 世界書裡 visibility＝Gm 的條目內容（標題照顯示）。
  - 「劇本」分頁整頁。
- **揭露例外**：鎖住時，`revealed.<id>` 為 true 的秘密照樣顯示在「劇本」分頁的「已揭露」清單（附揭露的章號）。
- **不看就重骰**：
  - 在「劇本」分頁按：整份聖經重生成（2.2）。
  - 在角色私設按：新呼叫，只重生該角色的 PRIVATE，以及 `holder` 是他的秘密。輸出用 `## PRIVATE` 段加 ```json 秘密陣列，替換該角色的 `private_md` 與那幾條秘密，其他不動。
  - 新舊秘密 id 的 `revealed` 葉子與規則要同步：舊 id 刪除，新 id 建成 false 並補上規則。走 2.2 同一個套用函式。
  - 重骰期間與重骰後內容都不顯示，只提示「已重骰」。
  - 重骰只限未遊玩的桌（2.3），所以重骰後私設立即生效，不會在幕中動到 system。
  - 解鎖後手改私設：照既有角色編輯，立即生效、不做凍結。總綱 §4 寫的「改私設下一章生效」在此改掉。
- **GM 世界書條目**的遮罩只有「解鎖」：匯入的內容沒有東西可以重骰。
- **genesis 失敗畫面**：VN 桌 expand 解析失敗時，`GenerateTableDialog` 不顯示原始輸出（原文含私設）。

### 2.6 「劇本」分頁與精靈 ②

- **`WorldEditor` 加頁籤**：只在 VN 桌出現，兩頁「世界｜劇本」，照 `CardEditor` 立繪分頁的切換寫法。
- **劇本分頁可改的欄位**：
  - 章數上限 M（數字，夾 3–30）。M 不算劇透，鎖住時也露在遮罩外面。
  - 主線、文風、各章文字欄、秘密文字與揭露時機、旗標說明、結局標題。
  - 頁上寫明「改動下一幕生效」。
- **劇本分頁不可改的欄位**：結構類（增刪章、旗標、結局，改 id 與條件）在包 4 只能看，要改就重骰。
- **頁內清單**：
  - 「旗標現值」清單（讀 `state.json`）。
  - 「已揭露」清單。
  - 「重生成聖經」鈕。
- **精靈 ①**：在 `GenerateTableDialog` 的 toolbar 槽（VnWizard 已在用）加勾選框「讓劇情給我驚喜（鎖住劇透）」，預設勾。匯入路線的 ① 選卡畫面也放同一個勾選框。
- **精靈 ②**：換掉佔位，genesis 與匯入兩條路都會進來。
  - 一進來就呼叫聖經生成，顯示進度。
  - 完成後顯示章數 M 與理由（可改），其餘內容依鎖的狀態顯示遮罩或全文。
  - 失敗時顯示「沒有聖經也能玩」，以及「重試」與「繼續」兩顆鈕。
  - 接著進 ③。

### 2.7 旗標提示（總綱 §7 UI）

- 播放到某一則事件的第一格時，如果該事件快照的 `changes` 有 `flags.*` 或 `revealed.*`，就在格邊顯示小提示：
  - 旗標：「◆ <label> +1」或「◆ <label>：<值>」。數值取 `changes` 的顯示標記（`+5`／`更新`）；標記是「更新」時，改取快照樹上的新值。
  - 揭露：「◆ 揭露：<秘密標題>」。
- 預設顯示，設定可關〔作者裁決 2026-11-01〕。開關放 VN 工具列的設定，存 app 設定。
- 劇透鎖住時，旗標提示照樣顯示（旗標變化本來就是劇情進行中發生的事），但不顯示未揭露秘密的標題。
- log 側欄的旗標面板改由「劇本」分頁的「旗標現值」清單代替，記為已知限制。

### 2.8 範圍外（後包）

`[chapter]`／`[ending]` 的規範、章結束流程、收束模式、結局判定都在包 5；VN 卡帶聖經在包 9；`chapter_lock` 在包 9。

## 3 檔案落點

- 後端：
  - `data/vn.rs`：結構、驗證正規化、讀寫、凍結、補種。
  - `vn_bible.rs`：生成與重試、套用到狀態樹與規則、角色秘密重骰。
  - `transport/vn_bible_prompts.rs`：生成提示詞、常駐段與當章包的組字。
  - `commands/vn_bible.rs`：`vn_bible_read`、`vn_bible_generate`、`vn_bible_save`、`vn_spoiler_set`、`vn_secret_reroll`。
  - `data/state.rs`：`FieldRule.allowed`，以及 apply 的拒收那處。
  - `chat_assembly.rs`：兩處注入。
  - `chat_turn/commit.rs`：補種。
- 前端 `features/vn/`：`SpoilerMask.tsx`、`BibleTab.tsx`、`useBible.ts`、`VnBibleStep.tsx`（精靈 ②）、旗標提示接進 `Stage` 或 `DialogueBox`。另外 `WorldEditor`、`CardEditor`、`WorldbookSection` 各加遮罩接點，i18n 補 `vn-mode.ts`。
- 熱點：`chat_assembly.rs` 704 行、`CardEditor.tsx` 729 行，注入與遮罩一律呼叫新檔裡的函式，熱點檔只加接點。

## 4 測試與實測

- 單元測試：
  - 驗證正規化：壞 id、壞條件、缺 `default`、`none` 補章門檻、M 夾值、超長截短。
  - `allowed` 拒收加回饋。
  - 補種只補不覆蓋。
  - 凍結換章才重抄。
  - 重骰整批換旗標。
  - 解析失敗重試一次後退 `null`。
  - 角色線組裝不含聖經 token。
- 測試通道真模型實測（截圖主線親看）：
  1. 一句話開 VN 桌，① 預設鎖 → ② 生成聖經 → 遮罩畫面與 M 可改 → ③④ 進桌。
  2. 玩 3–5 輪，看 AI log 裡 GM 是否以 `<UpdateVariable>` 改旗標或揭露，格邊提示是否出現，「劇本」分頁的旗標現值是否跟著變。
  3. 故意讓模型寫不合法的值，看下一輪回饋句（必要時改用 harness 注入）。
  4. 解鎖 → 看到全文 → 改一章目標 → 確認當幕 system 不變（AI log 的 system 雜湊或內容）。
  5. 私設遮罩按「不看就重骰」→ 內容不顯示但檔案已變。
  6. 匯入角色卡建 VN 桌 → ② 讀卡生成聖經。

## 5 觀察

（施工與實測發現的範圍外問題寫這裡，一行一條。）
- 實測 Haiku 當 GM 玩十多輪一次都沒寫 `<UpdateVariable>`（旗標表與更新指示都有送到）；換 gpt-5.6-luna 才會寫，低階模型的旗標遵守度要在包 5 結局判定前再看。
- luna 第一次把旗標寫成 `/trust_zhou`（漏 `flags/` 前綴），既有「路徑不存在」回饋句建議「新增請用 insert」，在 VN 桌會誘導模型在頂層長出表外鍵（頂層不在受保護根內）；下一輪已自癒寫對 `/revealed/…`。
- 匯入簡體卡（西幻）時聖經內容跟著卡寫成簡體，沒照介面語言 zh-TW；genesis 提示詞同樣只靠 BCP-47 代碼。
- 劇本分頁改了章數上限後，模型寫的「理由」仍是舊章數的說法（例：「八章足以…」配 M=6）。
