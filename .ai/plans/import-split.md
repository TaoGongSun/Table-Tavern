# import.rs 拆進 import/

分支：`refactor/import-split`  
立案基準：`main` / `02c7e1196bf246ddbd896e71d6023371c8d74bbb`  
原 `src-tauri/src/import.rs` blob：`1332ba62cd1541cdb6edf519bd8fc9b5c2286e84`

本案沿用 `data/`、`transport/`、`mechanism/`、`refactor_ai/` 已確立的拆檔規則：**純搬家、production body 逐 byte 不動、只允許 module plumbing 所需的 import／visibility 調整、`mod.rs` 只當 facade、零呼叫端 re-export 不掛**。不趁拆檔重寫 parser、不收斂重複碼、不改命名、不修既有行為。

本案採專案新的分支生命週期：立案書與工作分支一起建立；所有施工只進 `refactor/import-split`；驗收完成後以 squash 收回 `main`，再刪工作分支，讓 `main` 只留下單一乾淨結案 commit。

## 1. 施工前基準

`src-tauri/src/import.rs` 原檔共 **2847 行**：

- production：1–1499
- `#[cfg(test)] mod tests`：1501–2847
- production top-level item：**62 個**
  - `pub`：21
  - private：41
- `#[test]` leaf：**41 支**

完整施工前 manifest、caller inventory、test owner 與 visibility ledger 見：

- `.ai/baselines/import-split.md`

21 個既有 `pub` API：

`ImportProbe`、`probe_import`、`import_character`、`InterfaceScript`、`CardInterface`、`read_card_interfaces`、`save_world_card`、`save_gm_image`、`gm_image`、`card_format_entry`、`export_character`、`import_card_extension`、`import_mechanism`、`card_openings`、`character_image`、`save_character_image`、`delete_character_image`、`character_avatar`、`save_character_avatar`、`delete_character_avatar`、`worldbook_json`。

這 21 個都有 `import.rs` 外部實際 Rust caller／型別引用，因此拆後全部保留原 `import::...` 路徑；既有 caller 不因本案修改。

## 2. 定案切線

依實際依賴方向拆成 6 個 implementation 檔＋純 facade：

| 檔案 | 責任 |
|---|---|
| `card_io.rs` | PNG / base64 / JSON leaf helpers |
| `card.rs` | probe、角色卡匯入、開場白、markdown、worldbook 轉換 |
| `interface.rs` | 卡片介面腳本掃描、保存與格式條目辨識 |
| `mechanism.rs` | Table Tavern extension、MVU / EJS 機制匯入 |
| `export.rs` | SillyTavern V2 JSON / PNG 匯出 |
| `images.rs` | GM／角色圖／avatar 存取 |
| `mod.rs` | 只做 module 宣告與既有 API re-export |
| `test_support.rs` | 僅 cfg(test) 共用 `TestRoot`＋`minimal_png` |

### Production DAG

箭頭表示「左邊依賴右邊」：

```text
interface ─┐
images ────┼─→ card_io
mechanism ─┤
card ──────┼─→ card_io
card ──────┴─→ mechanism
export ──────→ card_io
export ──────→ card
export ──────→ mechanism
```

沒有 sibling cycle；`mod.rs` 不承擔 implementation 中繼層。

## 3. Visibility 白名單

施工前鎖定且施工後實際只有以下 **9 項**由 private 最小放寬為 `pub(super)`：

1. `card_io::PNG_MAGIC`
2. `card_io::string_field`
3. `card_io::decode_png_character`
4. `card_io::base64_encode`
5. `card_io::png_chunk`
6. `card_io::crc32`
7. `card::PUBLIC_SECTIONS`
8. `mechanism::table_tavern_extension`
9. `mechanism::import_table_tavern_extension`

沒有其他 production visibility 擴張；沒有新增 crate-level public API。

## 4. 測試搬法

原 41 支 tests 全部跟 owner implementation 搬：

- `card.rs`：12
- `mechanism.rs`：7
- `export.rs`：6
- `card_io.rs`：3
- `images.rs`：2
- `interface.rs`：11

`test_support.rs` 只收跨 owner 共用的 `TestRoot` 與 `minimal_png`。原測試函式 body 不重寫，只新增拆檔後必要的 test-module imports。

## 5. 本案白名單

允許：

1. `src-tauri/src/import.rs` → `src-tauri/src/import/` 的純搬家；
2. 新增 `mod.rs` 與上述 implementation 檔；
3. 必要 `use`、相對 module path、9 項既定 `pub(super)` plumbing；
4. `#[cfg(test)] test_support.rs`；
5. facade 維持原 21 個 API 路徑。

不允許且本案未做：

- 重寫 PNG / base64 parser；
- 抽新 helper 消除重複流程；
- 修改 SillyTavern 相容判定；
- 修改 MVU / EJS 規則解析；
- 修改錯誤字串、序列化欄位或匯出格式；
- 修改既有 caller API 路徑；
- 順手修 unrelated warning / bug。

## 6. 實際施工結果

已完成：

- 原 `src-tauri/src/import.rs` 刪除；
- 新增 `src-tauri/src/import/`：
  - `card_io.rs`
  - `card.rs`
  - `interface.rs`
  - `mechanism.rs`
  - `export.rs`
  - `images.rs`
  - `mod.rs`
  - `test_support.rs`
- `mod.rs` 維持純 facade；
- 21 個既有 `import::...` API 全數保留；
- 既有外部 caller **0 修改**；
- 實際 visibility 放寬與施工前 9 項 ledger 完全一致。

施工中嚴格 byte integrity 驗收曾抓到一項純格式偏差：`import_table_tavern_extension` 的原單行 signature 被展成多行。該偏差已還原為原本單行格式；函式 body 從未改動。這也證明本案不是只靠 compiler 綠燈判斷「差不多一樣」，而是實際以原始 immutable blob 做逐 item 對照。

## 7. 驗收結果

### A. 編譯／測試驗收：全綠

使用工作分支上的臨時 GitHub Actions 驗收 workflow，Windows runner 實際執行：

1. `npm ci` — success
2. `npm run build` — **success**
3. `cargo test`（`src-tauri`）— **success**
4. `RUSTFLAGS="-Dprivate_interfaces" cargo check --all-targets` — **success**

驗收 run：`33889826059`，結論 `success`。  
該臨時 workflow 已於驗收後刪除，不進最終 squash diff。

### B. Production／test byte integrity：全綠

另以臨時 integrity workflow，直接用：

`git show 02c7e1196bf246ddbd896e71d6023371c8d74bbb:src-tauri/src/import.rs`

作為 immutable source of truth，對拆後檔案做機械逐項比對。

最終 run：`33891037409`，結論 `success`：

- production top-level item：**62 → 62**
- item set：遺失 0、多出 0
- owner assignment：全部符合 baseline
- 9 個 visibility widen：全部且只有事先白名單項目
- 去除這 9 個 `pub(super)` plumbing 後，**62 個 production item 原文逐項一致**
- test leaf：**41 → 41**
- test owner assignment：全部符合 baseline
- **41 支 test function body 逐項一致**

該臨時 integrity workflow 也已刪除，不進最終 squash diff。

### C. Facade / caller 驗收：全綠

- `mod.rs` production 僅 module 宣告＋`pub use`；沒有實作內容。
- facade 提供原 21 個 public API，沒有漏掉，也沒有新增無用 re-export。
- `commands/character.rs`、`commands/world.rs`、`commands/image.rs`、`commands/refactor.rs`、`commands/chat.rs`、`receipts.rs` 等既有 caller 都未因拆檔修改。

### D. 實機驗收：全綠

用 09-05 00:23 打的 release 包（含 c3e73d5）實跑五項，都不觸發 AI 呼叫：

- 角色卡匯入＋角色圖：`WestFantsy.png` 進來成卡並顯示圖。
- 世界書匯入：`main_Furry Continent Lorebook_world_info.json` 與 Transfur 世界書各自落地成桌，收據標 `kind: worldbook`，未被誤判成角色卡。
- 角色圖／avatar：換圖後 `characters/<id>.png` 實際更新。
- 匯出：匯出檔 PNG 檔頭與 chunk CRC 正確，`chara` base64 解出 `chara_card_v2` 與世界書條目。
- 卡片介面：介面面板整頁渲染。

## 8. 20 分鐘自然工作段紀錄

本案依約採自然工作段，不一次吞完整個重構：

- 工作段 A：施工前複核＋baseline
- 工作段 B：`card_io` / `images` / `interface`＋所屬 tests
- 工作段 C/D：`mechanism` / `card` / `export` / facade 切換
- 工作段 E：完整 build/test、private-interface、62-item、41-test integrity 驗收

每段只處理相鄰責任，不擴張到拆檔以外的重構。

## 9. 結案狀態

**已結案**：squash 收回 `main`，工作分支已刪。

收尾時修掉拆檔自身造成的兩個 unused import warning——`export.rs` 的 `crc32`／`decode_png_character`、`interface.rs` 的 `json` 只被測試使用，從檔頂移進各自 `mod tests`。函式 body 未動，`cargo test` 535 passed、編譯零警告。
