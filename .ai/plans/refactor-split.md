# refactor.rs 拆進 refactor/

分支：`refactor-split`  
立案基準：`main` / `1543e3ddb5033a2673486e5d256a3b999b3ca3c2`  
原 `src-tauri/src/refactor.rs` blob：`860e4adc9e4f8fbe0b70f26e94306bdaefc61fb6`

本案沿用 `data/`、`transport/`、`mechanism/`、`refactor_ai/`、`import/` 已確立的拆檔規則：**純搬家、production body 逐 byte 不動、只允許 module plumbing 所需的 import／visibility 調整、`mod.rs` 只當 facade、零呼叫端 re-export 不掛**。不趁拆檔重寫 apply、不抽新 abstraction、不改錯誤字串、不改 serde 契約、不修既有行為。

本案只處理 `refactor.rs` 的物理拆分；`refactor_ai/`、`refactor_assemble.rs`、`commands/refactor.rs`、`receipts.rs` 等 caller 除非編譯證明 module plumbing 必須，否則不改。

## 1. 現況盤點

`src-tauri/src/refactor.rs` 原檔共 **2294 行**：

- production：1–769
- 空白分隔：770
- `#[cfg(test)] mod tests`：771–2294
- production top-level item：**20 個**
  - `pub`：9
  - private：11

9 個 production `pub` item：

1. `RefactorCharacter`
2. `RefactorInterface`
3. `RefactorMechanism`
4. `RefactorOutcome`
5. `RefactorSelection`
6. `RefactorApplySummary`
7. `RefactorApplyResult`
8. `apply`
9. `normalize_stored_mode`

11 個 private item：

- `PALETTE`
- `delete_source_entry`
- `absorbed_ledger_record`
- `absorbed_ledger_record_for_title`
- `normalize_interface_paths`
- `flatten_leaves`
- `unflatten`
- `is_empty_value`
- `shell_placeholders`
- `rebuild_state_fields`
- `json_to_state_node`

這個檔的「大」主要不是 production：**本體只有 769 行，約三分之二是 1524 行同檔測試**。因此施工目標不是把 2294 行平均切片，而是把 production 按責任拆乾淨，同時把測試從單一巨型 `mod tests` 拆成可維護的 owner 群組。

## 2. 外部 caller 現況

目前 code search 已確認：

| symbol | 外部 Rust caller／用途 |
|---|---|
| `RefactorCharacter` | `refactor_ai/types.rs`、`refactor_ai/result_parse.rs`、`refactor_assemble.rs` |
| `RefactorInterface` | `refactor_ai/types.rs`、`refactor_ai/result_parse.rs` |
| `RefactorOutcome` | `commands/refactor.rs` command input |
| `RefactorSelection` | `commands/refactor.rs` command input |
| `RefactorApplySummary` | `commands/refactor.rs` command return type |
| `apply` | `commands/refactor.rs` |
| `normalize_stored_mode` | `commands/refactor.rs` |
| `RefactorMechanism` | 暫未找到 refactor.rs 外直接 symbol caller；經 `RefactorOutcome.mechanisms` 間接暴露 |
| `RefactorApplyResult` | 暫未找到 refactor.rs 外直接 symbol caller；是 `apply()` 回傳型別，caller 直接取推導出的欄位 |

開工前 baseline 要再用當時 HEAD 重掃一次；facade 以**實際 caller**為準，不因「以前是 pub」就機械掛滿 9 個 re-export。

`RefactorMechanism`、`RefactorApplyResult` 目前列為零直接 caller 候選：預設不從 facade re-export；但必須跑 `RUSTFLAGS="-Dprivate_interfaces" cargo check --all-targets`。若 public signature 的 effective visibility 要求 re-export，才以最小必要 plumbing 保留，並在 baseline／施工結果記錄理由。

## 3. Production 定案草案

目前 production 只有三個真正責任群，不值得硬切成很多小檔。草案如下，**開工前仍以依賴 DAG 複核為最後定案**：

| 檔案 | 原責任／item |
|---|---|
| `types.rs` | `RefactorCharacter`、`RefactorInterface`、`RefactorMechanism`、`RefactorOutcome`、`RefactorSelection`、`RefactorApplySummary`、`RefactorApplyResult`、`normalize_stored_mode` |
| `apply.rs` | `PALETTE`、`apply`、`delete_source_entry`、`absorbed_ledger_record`、`absorbed_ledger_record_for_title` |
| `interface.rs` | `normalize_interface_paths`、`flatten_leaves`、`unflatten`、`is_empty_value`、`shell_placeholders`、`rebuild_state_fields`、`json_to_state_node` |
| `mod.rs` | module 宣告＋必要既有 API re-export；不放 implementation |

原檔視覺區段約為：型別 14–146、`apply` 主流程 147–488、來源刪除／帳本 helper 490–534、介面路徑與狀態樹 helper 約 535–755、stored mode 756–769。這些行號只用來定位，不作為依賴證據。

### Production DAG 草案

箭頭表示「左邊依賴右邊」：

```text
apply ─────→ types
  └────────→ interface

interface ─→ data::{FieldRule, StateNode}
types ─────→ data types
  ├────────→ refactor_ai::RefactorNewEntry
  └────────→ refactor_assemble::{RefactorDroppedEntry, RefactorUnabsorbedItem, RefactorAuditItem}
```

`apply.rs` 是唯一 orchestration owner；`interface.rs` 不反向呼叫 `apply.rs`，`types.rs` 不依賴 sibling implementation，所以預期無 refactor/ 內 sibling cycle。

### Visibility 草案

目前預期只有兩個 private helper 因 sibling 呼叫需要最小放寬：

1. `interface::normalize_interface_paths` → `pub(super)`
2. `interface::rebuild_state_fields` → `pub(super)`

其餘 private item 應保持 private；開工複核若需要新增 `pub(super)`，必須先更新 visibility ledger，不能邊編譯邊隨手放寬。

## 4. 測試拆法

原測試 1524 行，會是本案維護收益最大的部分。**不按原行號平均切**，改按行為 owner 分組；測試函式 body 不重寫，只搬家＋補必要 import。

建議結構：

```text
src-tauri/src/refactor/
  mod.rs
  types.rs
  apply.rs
  interface.rs
  test_support.rs          # cfg(test) 共用 TestRoot／seed helpers
  tests/
    mod.rs
    characters.rs          # 升格、玩家卡、source ownership／刪除、selection
    interface.rs           # path normalize、shell、state tree、mode
    mechanism.rs           # mechanism rules/triggers、ledger、undo
    entries.rs             # entries/meta、dropped、legacy serde、outcome persistence
```

現有共用測試 helper 至少有：`TestRoot`、`seed_entry`、`character`、`no_player_selection`、`apply_recorded`；它們進 `test_support.rs`，避免四個 test owner 互相 import。

開工 baseline 必須先列出所有 `#[test]` leaf 名稱並分配 owner；若某支測試同時跨兩責任，依「主要被驗證行為」歸檔，不拆 test body。

若 owner inventory 顯示某組仍超過約 700 行，再在同責任內拆第二層；不要為湊行數提前製造碎檔。

## 5. 開工前必做 baseline

新增 `.ai/baselines/refactor-split.md`，以 immutable blob `860e4adc9e4f8fbe0b70f26e94306bdaefc61fb6` 為 source of truth，至少鎖四份資料：

1. **production manifest**：20 個 top-level item 的名稱、種類、可見度、原行號區間、body hash；
2. **caller inventory**：所有 `refactor::...` 外部 Rust symbol caller，決定 facade re-export 白名單；
3. **test manifest**：所有 `#[test]` leaf 名稱 multiset、owner 分組、函式 body hash；
4. **visibility ledger**：所有拆檔後需要由 private → `pub(super)` 的項目，預期基線只有上述 2 項。

另外畫一次實際 production call DAG；若複核發現 `interface.rs` 與 `apply.rs` 有反向依賴，先調整 owner，再施工。禁止用 `mod.rs` 塞 implementation 來掩蓋 cycle。

## 6. 施工白名單

允許：

1. `src-tauri/src/refactor.rs` → `src-tauri/src/refactor/` 純搬家；
2. 新增 `mod.rs`、`types.rs`、`apply.rs`、`interface.rs`、測試分檔與 `test_support.rs`；
3. 必要 `use`、module path、事先 ledger 的最小 `pub(super)`；
4. facade 保住所有有實際 caller 的原 `refactor::...` 路徑；
5. 測試只改 module plumbing，test function body 不改。

不允許：

- 拆 `apply()` 成多個新 helper；
- 重寫來源消耗／刪除判定；
- 改介面雙套鏡像折疊算法；
- 改 state tree rebuild 行為；
- 改 mode 正規化規則；
- 改 serde default／skip 行為；
- 改錯誤訊息；
- 順手修 refactor-card-png-export 等後續功能；
- 修改 caller 來配合新路徑（應由 facade 相容）；
- 順手清 warning／rename／format unrelated code。

## 7. 驗收

### A. Byte integrity

以原 blob 機械比對：

- production top-level item：**20 → 20**；
- item set 遺失 0、多出 0；
- 去除事先白名單的 visibility／module plumbing 後，20 個 production item 原文逐項一致；
- test leaf 名稱 multiset 完全一致；
- 每支 test function body hash 一致。

### B. Facade / caller

- `mod.rs` production 只有 module 宣告與 re-export；
- 所有原有外部 caller 不修改；
- 零 caller symbol 不為了「看起來完整」額外 re-export；
- `RUSTFLAGS="-Dprivate_interfaces" cargo check --all-targets` 全綠。

### C. 編譯／測試

至少跑：

1. `cargo test`（`src-tauri`）
2. `RUSTFLAGS="-Dprivate_interfaces" cargo check --all-targets`
3. `npm run build`（確認 Tauri command 契約未被 Rust 拆檔意外破壞前端 build）

本案不改行為，因此不要求新增測試；若搬家本身暴露既有未測死角，另立案，不在本案擴張。

## 8. 自然工作段

依專案約定，以約 20 分鐘自然工作段施工，不一次吞完整案：

- **A：依賴複核＋baseline**——20-item manifest、caller、test owner、visibility ledger、DAG；只寫 `.ai/`。
- **B：production 搬家**——`types`／`interface`／`apply`／facade，先過 cargo check；不動 tests body。
- **C：tests 搬家**——共用 support＋四個 owner，跑 cargo test。
- **D：integrity＋完整驗收**——immutable blob byte/hash 比對、private-interface、cargo test、npm build；只修拆檔自身 plumbing。

每段完成後回報實際結果與下一段，不自動擴張到其他重構。

## 9. 結案狀態

**已完工併回 main。**

- `refactor.rs` 2294 行 → `refactor/` 六檔，最大 615 行
- 20 個 production item、33 支測試 body 逐字一致；visibility 放寬只有 ledger 上的兩項
- 收尾清掉拆檔帶進來的 8 個警告：四支測試檔照抄的 `use` 只留各自用到的型別；facade 不 re-export 零 caller 的 `RefactorApplyResult`、`RefactorMechanism`（照本檔 §7-B），改由 `test_support` 與 `tests/mechanism` 直接從 `types` 取
- 驗收：`cargo test` 535 passed 零警告、`RUSTFLAGS="-Dprivate_interfaces" cargo check --all-targets` 綠、`npm run build` 綠、實機跑過匯出重構卡→匯入→套用（角色卡 18→36 張落檔）
