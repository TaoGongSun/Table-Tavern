# refactor-statusbar-skeleton — 計畫

交接：[handoffs/refactor-statusbar-skeleton.md](../handoffs/archive/refactor-statusbar-skeleton.md)

## 核心原則

重構的目的是把固定格式交給 app 接管，省模型輸出；任何會讓模型重新輸出固定格式的設計都不採用〔作者裁決 2026-10-02〕。

## 定案範圍

- 狀態欄型（playable: no）的介面條目也產骨架，app 用每一樓的狀態快照填原卡狀態區塊，再交給原卡畫面顯示〔作者裁決 2026-10-02〕。
- 「不接管時保留介面來源條目」已作廢〔作者裁決 2026-10-02〕。
- 純文字（沒有卡顯示腳本）的 no 條目也產骨架、開增量；逐樓合成先量效能，慢了再改依事件快取〔模型判斷·未裁決，主線決定照建議〕。
- 大改前的桌與舊產物不做相容〔作者裁決 2026-10-02〕。

## 做法（已實作）

### 1. 展開分類（後端 `refactor_ai`）
- `EntryKind` 改成兩種：`InterfaceShell`（playable: yes）與 `InterfaceStatusbar`（playable: no，前端字串 `interface_statusbar`）。只抽 STATE 的 `Interface` 刪除。
- 兩者都產 STATE＋SHELL＋RULES＋GUIDE，共用狀態規則、照搬規則（`INTERFACE_SHELL_COPY_RULES`）、更新規則、欄位 schema 與解析器；只差骨架規格的開頭段（`INTERFACE_SHELL_PLAYABLE_INTRO`／`INTERFACE_SHELL_STATUSBAR_INTRO`）。
- kind 專屬規則在 user 訊息，共用 system 不動，快取紅線不受影響。`expand_messages` 拿掉不再用到的 `lang` 參數。
- SPLITS route=statusbar 的入口 `refactor_expand_spans` 改用 `SPANS_EXPAND_KIND = InterfaceStatusbar`（有入口測試），混合條目不會再退回只抽 STATE。
- EntryKind 只是呼叫分類：玩法 mode、產物的 `interface` 欄位、收據、匯出入格式都不跟著改。

### 2. 產物完整性（`result_parse.rs`）
兩種 kind 同一份契約，任一項不成立就回 None：該條記成失敗，來源條目不被消耗，絕不落成「只有 STATE」的桌。
- STATE 是 JSON 物件。
- SHELL 非空（截斷在骨架中間時，後面的 GUIDE 必然缺席而失敗）。
- RULES 區塊存在、非空，而且是合法 JSON。
- GUIDE 非空。
- 每個動態佔位符（`{{本回合.正文}}` 除外）都是 STATE 的葉子，而且有一條欄位規則。

### 3. 多介面合併（前端 `mergeRefactorInterfaces`）
- 依最小來源 uid 穩定排序，平行展開完成的順序不影響結果。
- state_fields 與 rules 淺合併（排序在後者蓋前）。
- 骨架與指引各自去重（trim 後全文相同）後依序串接：骨架以換行接起，指引空一行接起。不再只取最後一份。

### 4. 每一樓各自填骨架（前端 `card-shell-route.ts`）
- 選路先算出每一樓交給卡片的文字，本樓、歷史樓、初次掛載、推送都用同一份結果，不再用 `tableTree` 填本樓。
- 有骨架時，只有 GM 旁白／角色對話樓會合成：
  - 那一樓原文自己畫得出殼（重構前的開場白、舊回合），就用原文。
  - 否則用這一樓的 `state.tree` 加這一樓的正文填骨架。
  - 沒有快照時用原文。
- 玩家與 system 樓一律原文；合成只在記憶體裡，不寫回逐字稿。
- 選殼：最新一樓先試，往前最多 10 樓；空桌退回卡片開場白。最新一則是 system 時，本樓會落在前一個 GM 樓。

### 5. YAML 值的表示（`refactor-shell.ts`）
- 卡的顯示腳本含 js-yaml／`yaml.load`／`YAML.parse` 時，用 `yaml` 格式填值，否則照舊原樣放回（`raw`）。
- `yaml` 格式下，值依佔位符所在位置表示：
  - 雙引號字串裡：跳脫 `\`、`"`，換行寫成 `\n`。
  - 單引號字串裡：`'` 寫成 `''`，換行換成空白。
  - 佔位符是整個值（`鍵: {{x}}`、`- {{x}}`、或整行只有佔位符）：
    - 多行 YAML 結構照結構縮排插入。
    - 多行文字寫成 `|-` 區塊。
    - 單行時，保留字、空值或含 YAML 特殊字元就加雙引號，其餘原樣（數字維持數字）。
  - 夾在其他固定文字中間：換行換成空白。
- 固定文字（地圖矩陣、白名單）逐字保留。
- 行內集合：只有欄位規則 `kind: list` 的欄位（套用時記進 `mechanism.value_types` 的 `list`）填在 `[{{x}}]` 唯一內容時當集合片段，契約見交接檔。其餘一律以完整元素表示（單一佔位符遵守型別表；前後綴組成的元素當字串；含 `,[]{}` 或換行加引號）。
- js-yaml 4.3.2 是正式依賴（4.x 已知公告全部修補，含原型污染 GHSA-mh29-5h37-fv8m）：片段用 core schema 解析。測試用它核對卡片讀到的值；卡片載入的 4.1.0 對全部案例讀到的結果相同，所以不另留舊版。

### 6. 收據補強（`receipts.rs`、`data/state.rs`）
- 收據的機制差異新增 `guide_before` 與 `value_types_before`（整份原表），undo 寫回原本的卡專屬指引與型別表〔模型判斷·未裁決，Sol 驗收要求〕。
- `Mechanism::is_empty` 把 guide 算進內容。原本只剩 guide 時整塊略過不寫，指引會悄悄消失，undo 寫回指引時踩到。

### 7. Sol 驗收必改（2026-10-03）
- 合併改成遞迴保留不同葉子，值或規則衝突與合併後佔位符對不上都算衝突，整組不套、列入失敗清單。
- YAML 表示只用在「實際解析 YAML 的那支腳本」的容器標籤內，依完整純量與區塊上下文表示（細節見交接檔）。
- 佔位符契約前後端共用：正文槽、原卡巨集、狀態路徑三類。
- 選殼先試最新 GM 樓，再走十樓 fallback。
- 回合尾導演指示改成三態 `gm_turn_format`（見第 8 節）。另加零額度提示詞匯出入口 `dump_gm_lane_prompt`（ignored 測試）。

### 8. 原待問 1～4 的定案（2026-10-03）
- **接管桌輸出契約**〔作者裁決 2026-10-03〕：有骨架、不是 characters 桌時，回合尾用 `takeover_instruction`，要求正文＋只寫變動的 `<UpdateVariable>`，沒變動就不寫，不要 ```state。前端 `interfaceTakeover` 用同一依據，不顯示頂部狀態欄。其餘桌照舊。
- **present 缺席**〔模型判斷·未裁決〕：只有介面接管桌（同 `gm_turn_format` 依據）present 缺席時換幕不結算角色卡隱藏，其他桌照原行為。
- **本桌更新範例**〔作者裁決 2026-10-03〕：system 在本桌 guide 後附本桌真實路徑的 delta／replace 範例（`table_update_example`，路徑來自欄位規則、照 JSON Pointer 跳脫）。通用協定原文不動。
- **外框條目**〔作者裁決 2026-10-03；判定方式〔模型判斷·未裁決，主線同意〕〕：
  - 後端 `frame_candidate_tags` 判候選（嚴格：容器內只有空白與佔位符；容器外沒有佔位符、沒有縮排「鍵: 值」），盤點結果帶 `frame_candidates`。
  - 前端 `confirmFrames`：要有定義骨架含相同容器標籤才當外框，否則照常展開。
  - `composeFrames`：依外框容器順序組殼，對上的容器搬入，唯一沒對上的容器放正文槽；組不起來就記失敗、保留來源。
  - 外框不產欄位、規則與指引，來源隨介面消耗。
- **開場白初始值**〔作者裁決 2026-10-03；做法 (a)〔模型判斷·未裁決，主線同意〕〕：玩家貼出的那則開場白（匯入原檔＋序號）裡、與條目同標籤的容器區塊，帶進 expand 的 user 訊息，初始值以它為準、欄位要能原樣表示這些值。system 不變。
- **重新重構**〔作者裁決 2026-10-03；原卡檔與清回細節〔模型判斷·未裁決，主線同意〕〕：
  - 兩條匯入路徑：驗卡→寫 `import-pending-<id>`（寫不進就不做）→存 `import-source-<新 id>.*`→匯入→記收據（識別掛收據）→都成功才刪標記（`import/files.rs`、`receipts/sources.rs`）。匯入回傳識別，前端貼開場白時帶回，序號只掛到那筆收據；撤銷時一起刪。
  - 匯入、撤銷、貼開場白、套用重構、改名補記整段持整桌獨占（`data::world_exclusive_async`，資料函式要出示 `&WorldExclusive`），快照、變更、記帳、復原之間不會插進別的寫入。
  - 貼開場白失敗：先拍逐字稿與 state.json 原始位元組，失敗時寫回並讀回確認原樣才解除標記，回不去就留著（`data::opening_checkpoint`）。
  - `rerun_status` 分 fresh／played／no_source（含來源不完整）／ready。
  - `reset_to_import_source` 整段持整桌獨占鎖，在臨時資料根 `worlds/.tt-reset-<id>` 依收據順序重匯（`import_worldbook_file`／`import_character_file`，與 command 共用）並重貼開場白，驗過（無未完成標記、重放清單逐筆一致）才用格式機制新增的 `reset` 操作原子交換（`replace_world_from_build`，恢復表另加 reset 四列）；失敗時原桌不動。
  - 前端按重構先問狀態：played／no_source 提示擋下，ready 確認後重置、刷新、照一般流程走。

## 已知限制

- 純數字值讀成 YAML 數字（與卡原文數值欄一致），其餘值卡讀到的都是原字串。
- 逐樓合成每次 events 變動都對每個 GM 樓跑一次卡 regex；本次實測（數樓）沒有感覺到延遲，沒有量長場景。

## 測試（已補）

- **Rust：**
  - 兩種 kind 的提示詞（共用規則、開頭段差異、system 逐位元組相同）。
  - spans 入口用狀態欄型。
  - `EntryKind::parse`（`interface` 不再接受）。
  - 完整產物解析，以及 13 種不完整產物 × 兩種 kind 一律失敗。
  - 狀態欄 YAML 骨架保留固定文字。
  - 狀態欄型新產物「匯出→匯入→套用→undo」整條路：來源、殼、incremental、rules、guide、mode、狀態樹。
  - undo 寫回第一輪指引。
- **vitest：**
  - 選路：本樓用自己的快照；歷史樓兩個 GM 樓值不同；system 與玩家不合成；沒快照退原文；沒重構過的桌不變；YAML 格式偵測與跳脫。
  - 合併：亂序輸入結果相同，骨架與指引去重串接。
  - YAML 填值：一般、特殊字元、空值、多行（引號內、YAML 清單、多行文字、整行佔位符），用 js-yaml 解析核對；固定矩陣保留；raw 不變。
  - controller：骨架本樓推送後保留；無關 render 不重掛。

## 驗收

實測結果見交接檔。
