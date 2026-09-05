# data/scene.rs 拆進 data/scene/：立案計畫

分支：`scene-split`  
立案基準：`main` / `d6a5be0cb5b835bef1684b060644a1525c4e9b45`  
原 `src-tauri/src/data/scene.rs` blob：`e7ccf217d30977f401afc8f982f24b911f7078f9`

本案沿用 `data/`、`transport/`、`mechanism/`、`refactor_ai/`、`import/`、`cli/` 已確立的拆檔規則：**純搬家、production body 逐 byte 不動、只允許 module plumbing 所需的 import／visibility／相對路徑調整、`mod.rs` 只當 facade、零呼叫端 re-export 不掛**。不趁拆檔改逐字稿格式、不改狀態快照回捲、不改換幕／分岔語意、不改 Markdown 匯出、不改角色卡自動上下場判定。

本輪只立案，不修改任何 `.rs`。下一階段施工前先做 immutable baseline、完整 caller inventory、test owner manifest 與 visibility ledger；本文件的切線是依目前實際符號依賴做的施工草案，仍以開工前機械複核為準，不能只因原檔行號相鄰就硬切。

## 1. 為什麼現在拆

`src-tauri/src/data/scene.rs` 共 **1776 行**；這個數字在先前 `.ai/plans/data-split.md` 的結案統計已確認：

- production：1–639，共 **639 行**
- 空行：640
- `#[cfg(test)] mod tests`：641–1776，共 **1136 行**（原 data-split 結案表以「測試區 1137」記錄時包含分隔空行）
- scene 測試 leaf：**25 支**

它不是 production 特別巨大，而是「四種不同責任＋一大包同檔 tests」長在一起：

1. 逐字稿 JSONL I/O、事件型別與 state snapshot 回捲；
2. 整桌／單幕 Markdown 匯出；
3. 場景 label、分岔、換幕、退幕、摘要重寫；
4. 角色卡／人物共用的登場掃描原語與換幕自動隱藏結算。

先前 `data.rs → data/` 拆分時，`scene.rs` 已被明確列成獨立領域並保留上述全部責任；那份計畫沒有再替 `scene.rs` 訂第二層切線。本案就是接續那次拆分，把目前 data/ 裡最大的檔案再依內部責任拆開。

## 2. 施工前基準：必做，不得省略

目前 production top-level item 初步盤點為 **28 個**：

- `pub`：17
- `pub(crate)`：4
- private：7

### 17 個既有 `pub`

`TranscriptKind`、`TranscriptEvent`、`scene_label`、`append_transcript`、`append_opening`、`sync_scene_state_tree`、`pop_transcript`、`remove_transcript_event`、`set_last_transcript_state`、`read_transcript`、`CARD_ARRIVAL_PREFIX`、`export_transcript_markdown`、`export_scene_markdown`、`fork_scene`、`begin_next_scene`、`revert_scene`、`replace_scene_summary`。

### 4 個既有 `pub(crate)`

`bracket_title`、`appeared_titles`、`split_present_names`、`name_matches`。

### 7 個 private

`transcript_path`、`rewrite_scene`、`render_transcript_entry`、`render_scene_section`、`format_scene_summary`、`next_scene_version`、`settle_card_visibility`。

開工前要從基準 blob 機械產生正式 manifest，逐項記錄：

- 名稱／種類／原 visibility；
- 原始行號區間；
- 原文 hash／byte slice；
- 施工後 owner 檔案；
- 是否允許 visibility 調整。

另建立：

1. **caller inventory**：搜尋 `scene.rs` 外 Rust caller，決定 facade 必須保留哪些 `scene::...` 路徑；
2. **test manifest**：25 支 `#[test]` leaf 的名稱、原 body hash、新 owner；
3. **visibility ledger**：所有因 sibling 呼叫而需要放寬的 private 項目先列白名單；
4. **dependency DAG**：依 production 真實符號引用定案，不靠行號鄰近。

施工後用 `scripts/split-verify/` 既有工具或等價機械檢查，對 immutable source blob 做逐 item／逐 test body 驗證。

## 3. 已確認的外部路徑約束

目前已確認：

- `data/mod.rs` 對外 `pub use scene::{...}` 目前列出上述 **17 個 public API**；
- `data/mod.rs` 另以 `pub(crate) use scene::{appeared_titles, name_matches, split_present_names};` 提供 crate 內共用，並另有 `#[allow(unused_imports)] pub(crate) use scene::bracket_title;` 保留既有 facade 路徑；
- `data/world.rs` 直接從 `super::scene` 使用 `TranscriptEvent`、`TranscriptKind`、`append_transcript`；
- `transport/arrivals.rs` 經 `data::appeared_titles`、`data::name_matches` 等共用登場判定；
- `commands/scene.rs` 也使用 `data::appeared_titles`／`data::name_matches` 與逐字稿 API。

因此本案目標是：**`data/mod.rs` 原則上 0 修改，scene 內部怎麼拆都由新的 `data/scene/mod.rs` 維持現有路徑。**

`bracket_title` 目前是 `pub(crate)`；雖然沒有找到 `data/mod.rs` 之外的直接 consumer，`data/mod.rs` 本身已有 `#[allow(unused_imports)] pub(crate) use scene::bracket_title;`，因此新的 scene facade **必須繼續 re-export** 它，才能維持 `data/mod.rs` 0 修改與既有 facade 路徑。這項漏盤點在 presence 拆分第一次 `cargo test` 時由 compiler 抓出，之後已補正；implementation 定義仍維持原 `pub(crate)`，沒有額外 visibility widen。

## 4. 草案切線：4 個 implementation 檔＋純 facade

| 檔案 | 責任 | production item |
|---|---|---|
| `transcript.rs` | 事件型別、JSONL 路徑/I/O、state snapshot 寫入與回捲 | `TranscriptKind`、`TranscriptEvent`、`transcript_path`、`append_transcript`、`append_opening`、`rewrite_scene`、`sync_scene_state_tree`、`pop_transcript`、`remove_transcript_event`、`set_last_transcript_state`、`read_transcript` |
| `export.rs` | 單事件渲染、場景段落、整桌／單幕 Markdown 匯出 | `render_transcript_entry`、`render_scene_section`、`export_transcript_markdown`、`export_scene_markdown` |
| `presence.rs` | 登場標記解析、present 名單比對、角色卡換幕 visibility 結算 | `CARD_ARRIVAL_PREFIX`、`bracket_title`、`appeared_titles`、`split_present_names`、`name_matches`、`settle_card_visibility` |
| `lifecycle.rs` | scene label／version、fork、begin、revert、summary rewrite | `scene_label`、`format_scene_summary`、`next_scene_version`、`fork_scene`、`begin_next_scene`、`revert_scene`、`replace_scene_summary` |
| `mod.rs` | module 宣告＋既有必要 API re-export | 不放 implementation |

### 為什麼不拆 `types.rs`

`TranscriptKind`／`TranscriptEvent` 只有約數十行，而且它們的生命週期與 JSONL transcript I/O 強綁。單獨抽 `types.rs` 只會多一層所有檔都要依賴的薄模組，沒有解除依賴或 ownership 的實際收益。

### 為什麼把 presence 與 lifecycle 分開

登場掃描原語不只換幕自己用，`transport/arrivals.rs` 與 `commands/scene.rs` 也把它當跨領域共用規則；另一方面 `fork_scene`／`begin_next_scene`／`revert_scene` 是場景歷史 DAG 的生命週期操作。兩者現在只是因 `begin_next_scene` 結尾會做一次 visibility settlement 才相接，拆開後依賴可以維持單向，不需要把 transport 反向拉進 data。

## 5. 預期 dependency DAG

箭頭表示「左邊依賴右邊」：

```text
export ───────→ transcript
presence ─────→ transcript
lifecycle ────→ transcript
lifecycle ────→ presence
```

外部 sibling 依賴：

- `transcript.rs` → `paths`、`state`，另用 crate-level `mechanism`／`transport::StateBlock`；
- `export.rs` → `paths`、`state`／`local_timestamp`、`transcript`；
- `presence.rs` → `character`、`transcript`；
- `lifecycle.rs` → `state`／`local_timestamp`、`transcript`、`presence`。

預期 **零 sibling cycle**，`mod.rs` 不承擔 implementation 中繼邏輯。

## 6. Visibility 白名單草案

若上述切線在開工前 DAG 複核後維持不變，預期只有 **2 個** production private item 需要最小放寬為 `pub(super)`：

1. `transcript::transcript_path`  
   - 原因：`export.rs` 要判斷單幕檔是否存在；`lifecycle.rs` 的 fork／revert／replace 直接讀寫同一路徑。
2. `presence::settle_card_visibility`  
   - 原因：`lifecycle::begin_next_scene` 在換幕完成後呼叫一次結算。

其餘 private 項目應留在 owner 檔內：

- `rewrite_scene` 留在 `transcript.rs`；
- `render_transcript_entry`／`render_scene_section` 留在 `export.rs`；
- `format_scene_summary`／`next_scene_version` 留在 `lifecycle.rs`。

**正式白名單要在施工前 baseline/DAG 完成後定案；沒有列進 ledger 的項目不得施工中臨時放寬。** 若實際編譯證明還需要第三項，先更新本計畫與理由再繼續，不用 `pub(crate)` 一把梭。

## 7. 測試搬法

原 **25 支** scene tests 不整包塞進 `mod.rs`，依驗證責任搬到各 implementation 檔的 nested `#[cfg(test)] mod tests`。既有 `data/test_support.rs` 繼續共用，不新增第二套 scene 專用 fixture。

草案 owner：

- `transcript.rs`：**8 支**
  - JSONL round-trip／invalid kind
  - pop 行為
  - append snapshot
  - opening raw／state
  - pop snapshot restore
  - undo restore
  - nested tree snapshot restore
- `export.rs`：**4 支**
  - 全桌雙語匯出
  - 空桌拒絕
  - 單幕匯出
  - missing scene 拒絕
- `lifecycle.rs`：**12 支**
  - begin next scene 2
  - revert 3
  - replace summary 3
  - fork 3
  - scene_label fallback 1
- `presence.rs`：**1 支**
  - `begin_next_scene_settles_card_auto_hidden`

硬約束：

1. 原 `#[test] fn` body 逐項保持一致；
2. 只允許拆檔必要的 test imports／路徑 plumbing；
3. 不趁搬家改 fixture、合併案例、改名稱或「整理」斷言；
4. 25 支 leaf 施工前後名稱 multiset 與 body hash 都一致。

若依賴複核發現某支測試更適合跟另一 owner，允許調整落點，但只搬整支 test，不改 body。

## 8. 本案允許與禁止

### 允許

1. `src-tauri/src/data/scene.rs` → `src-tauri/src/data/scene/` 的純搬家；
2. 新增 `mod.rs`、`transcript.rs`、`export.rs`、`presence.rs`、`lifecycle.rs`；
3. 拆檔後必要的 `use`、相對 module path；
4. 事前 visibility ledger 核准的最小 `pub(super)` plumbing；
5. tests 跟 owner 搬家；
6. facade 維持現有 `scene::...`／`data::...` caller 路徑。

### 禁止

- 改 JSONL 欄位、serde rename/default/skip 規則；
- 改 `TranscriptKind`／`TranscriptEvent` 對外形狀；
- 改 append 時自動補 state snapshot 或 cache 回寫行為；
- 改 pop/remove/rewrite 的 state 回捲規則；
- 改 opening 的 `mechanism::apply_block` 流程；
- 改 Markdown 標點、標題、語系文字、排序或錯誤字串；
- 改 scene label base/version/parent/forked 算法；
- 改 fork／begin／revert／replace 的守門條件與副作用順序；
- 改 `CARD_ARRIVAL_PREFIX`、present 斷詞、雙向包含 name match；
- 改 auto_hidden settlement 的 archived／arrival／present 判定；
- 抽新 trait、統一 helper、消除重複碼；
- 順手修 unrelated bug／warning。

若施工中發現既有 bug，另立案，不夾在 scene 拆分 commit。

## 9. 驗收門檻

最低驗收：

1. `cargo test`
2. `RUSTFLAGS="-Dprivate_interfaces" cargo check --all-targets`
3. `npm run build`
4. production top-level item：**28 → 28**，item set 零遺失零新增
5. 除正式 visibility ledger 白名單外，production item 原文逐項 byte-identical
6. test leaf：**25 → 25**，名稱 multiset 與 test body hash 全一致
7. `data/scene/mod.rs` production 只含 module 宣告／必要 re-export，無 implementation
8. `data/mod.rs` 原則上 **0 修改**
9. facade 必須保住目前 17 個 public API 與實際有 caller 的 crate API；零 caller 項目不額外挂 re-export
10. `scene.rs` 外 caller 原則上 **0 修改**；若 compiler 證明現有路徑無法透過 facade 保持，先停下更新計畫

### 行為 smoke

因本檔全是本地資料／場景流程，結案前至少驗：

- append 一則事件後讀回，state snapshot 正常；
- pop 一次後 state 回到上一則快照；
- 整桌與單幕 Markdown 能匯出；
- begin next scene 產生摘要並推進幕號；
- revert 在只有摘要時能退、已有新內容時會擋；
- 從前幕 fork 後原幕不動、label/version 正確；
- 有角色回歸事件或 present 命中的卡在換幕後不被 auto-hide，archived 不被自動改。

這些優先由既有 25 支 tests 覆蓋；不為拆檔另造新行為測試，除非驗收發現現有測試完全沒有覆蓋某條 facade/plumbing 路徑。

## 10. 建議自然工作段

依專案約定，以約 20 分鐘的自然工作段推進，不一次吞完整檔：

- **A：依賴複核＋baseline**  
  正式 28-item manifest、完整 caller inventory、25-test manifest、DAG、visibility ledger；若草案切線有誤，先更新本計畫。
- **B：`transcript.rs`**  
  搬事件型別＋JSONL I/O＋8 tests，先讓核心逐字稿路徑編譯／測試通過。
- **C：`export.rs`＋`presence.rs`**  
  搬 Markdown 與登場／結算規則＋5 tests，確認 sibling visibility 沒超出 ledger。
- **D：`lifecycle.rs`＋facade 切換**  
  搬 scene DAG 操作＋12 tests，建立純 `mod.rs`，刪除原 `scene.rs`。
- **E：完整驗收**  
  cargo/npm/private-interface、28-item byte integrity、25-test body integrity、facade/caller 檢查與 smoke。

每段只做相鄰責任；如果某段實際工作量偏小，可以自然延伸到下一個緊鄰責任，但不把 unrelated 重構一起帶進來。

## 11. 結案狀態

**已結案。** `scene.rs` 1776 行拆成 `transcript.rs` 679／`lifecycle.rs` 729／`export.rs` 276／`presence.rs` 145＋15 行 facade，DAG 單向零循環。

驗收（`scripts/split-verify/` 對 immutable blob `e7ccf217…`）：

- production item 28 → 28，遺失 0 多出 0，內容變更 0
- 可見度變更只有第 6 節白名單那 2 項：`transcript_path`、`settle_card_visibility` 升 `pub(super)`
- facade 21 個 pub 全供得出來，漏 0 多 0
- test leaf 25 → 25，body 逐 byte 全同；分布 transcript 8／export 4／presence 1／lifecycle 12
- `cargo test` 535 passed 零警告、`-Dprivate_interfaces cargo check --all-targets` 過、`npm run build` 過
- `data/mod.rs` 與所有 caller 零修改

第 9 節行為 smoke 實機全過：逐字稿讀回、收回回捲、整桌與單幕 Markdown 匯出、換幕、退幕守門、fork、換幕後角色卡自動下場判定。

實測另外抓到一個既有 bug（與拆分無關，`main` 上就有）：收回後復原，狀態欄停在收回後的舊值。根因是前端記憶體裡的事件不帶狀態快照——快照是後端寫檔時才補的——復原時送回去的那則是空的，後端只好拿回捲後的檯面當它的快照。依第 8 節不夾進拆分 commit，另以 `undo-restore-state` 一筆修在 command 層：append 之前先補好快照，補完那份回傳給前端。`data/scene/` 未動。
