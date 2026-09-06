# Rust inline tests 外移：repo hygiene 立案與施工計畫

分支：`repo-hygiene/rust-inline-tests`
立案基準：`main` / `83f2bb997ca1774320ca1e79d8277de476b9a83b`
立案日期：2026-09-06

## 0. 立案目的

本案是 repo hygiene 第二項：把 Rust 大檔尾端的 inline `#[cfg(test)] mod tests { ... }` 外移成同一模組底下的測試檔，讓 production 檔只保留 production code 與一行 test module 宣告。

這不是功能重構，也不是重新整理測試。施工時不得順手改 production 行為、測試敘述、assert、fixture、helper、import 方向或可見度；唯一目的就是把已經自然形成的大塊測試區從 implementation 檔拿出去。

本案刻意不一次全拆。先盤點、排序，之後每個自然工作段原則上只處理一支 production 檔；每支都獨立驗證、獨立 commit，避免一次搬很多測試後無法快速定位 regression。

## 1. 盤點方式與優先判準

這次不是看到 `mod tests` 就全部搬，而是優先處理「大檔 + inline tests 占比高 + module path 可以原樣保留」的檔案。

排序判準：

1. **主檔清理收益**：inline tests 占整檔比例越高越優先。
2. **檔案本身夠大**：先看 30 KB 以上 Rust 檔，再補看雖較小但 tests 已占一半左右的檔。
3. **外移風險低**：能用 `#[cfg(test)] mod tests;` 保留原 module path，現有 `super::*` / `super::super::*` 不必改。
4. **不製造新架構**：不趁機抽 shared fixtures、不改 production visibility、不把測試重新按題目分類。
5. **驗證可局部完成**：每支外移後都能跑該 module 測試，再跑全套 Rust tests。

## 2. 候選盤點與順位

### A 級：最適合先做

| 順位 | 檔案 | 目前大小 | inline tests 狀況 | 判定 |
|---|---|---:|---|---|
| 1 | `src-tauri/src/transport/assemble.rs` | 50,257 B | production 約只到前 140 行；後面約 1,100 行為 `mod tests` | **第一個施工目標**。清理收益最大，module path 可完全保持 |
| 2 | `src-tauri/src/transport/client.rs` | 50,266 B | `#[cfg(test)]` 約從 506 行開始，直到約 1,140 行檔尾 | **第二個施工目標**。tests 超過半檔，外移價值高 |
| 3 | `src-tauri/src/refactor_assemble.rs` | 49,731 B | 後半段是一整塊大型 inline tests | **第三順位**。production / tests 邊界清楚 |
| 4 | `src-tauri/src/mechanism/apply.rs` | 34,512 B | production 約到 410 行，後面為大量 apply 行為測試 | **第四順位**。純搬家風險低，能明顯縮短 implementation 檔 |
| 5 | `src-tauri/src/data/character.rs` | 33,950 B | production 約到 370 多行，後面為資料層測試 | **第五順位**。適合外移，但收益低於前四支 |

### B 級：檔案較小，但 test ratio 很高

這幾支不是 30 KB 級大檔，但 tests 已占約半檔，因此 A 級做完後仍值得整理：

- `src-tauri/src/cli/runner.rs` — 20,878 B；production 約 260 行，後半約一半是 tests。含 Unix process / shell 行為，施工時需額外注意 target-specific 驗證。
- `src-tauri/src/import/export.rs` — 17,924 B；production 約 210 行，後半約一半是 round-trip / PNG / mechanism tests。
- `src-tauri/src/genesis.rs` — 18,962 B；inline tests 約占三分之一，收益中等。
- `src-tauri/src/evaluator.rs` — 21,172 B；有清楚 inline tests，但占比低於上述候選。

### C 級：暫不優先

- `src-tauri/src/data/state.rs` 有 inline tests，但測試區不大；現在拆只會增加檔案數，主檔可讀性收益有限。
- 小型 Rust 檔若只有零星幾支 tests，本案不為了「形式統一」強迫外移。

### 大檔但不是本案目標

盤點時特別複核了數支 30–65 KB 級 Rust 大檔；下列檔案目前沒有 inline `#[cfg(test)] mod tests`，因此即使本身很大，也不屬於這一項 hygiene：

- `src-tauri/src/lanes.rs`
- `src-tauri/src/install.rs`
- `src-tauri/src/ejs.rs`
- `src-tauri/src/import/mechanism.rs`
- `src-tauri/src/data/worldbook.rs`
- `src-tauri/src/receipts.rs`
- `src-tauri/src/transport/state_view.rs`
- `src-tauri/src/transport/response.rs`
- `src-tauri/src/usage_report.rs`

因此本案不把「Rust 大檔拆分」和「inline tests 外移」混成同一件事；前者若還要整理，另立案處理。

## 3. 定案的外移形狀

以第一順位 `transport/assemble.rs` 為例：

```text
src-tauri/src/transport/
├── assemble.rs
└── assemble/
    └── tests.rs
```

`assemble.rs` 尾端由：

```rust
#[cfg(test)]
mod tests {
    // 原測試內容
}
```

改成：

```rust
#[cfg(test)]
mod tests;
```

測試內容搬到 `src-tauri/src/transport/assemble/tests.rs`。

這樣 module path 仍然是 `crate::transport::assemble::tests`，因此原本測試裡的：

- `use super::*;`
- `use super::super::test_support::*;`
- `use super::super::messages::*;`
- 其他 `super::super::*`

語意都不變，不需要為了搬檔去升 production item 的 visibility，也不需要改 caller。

其他檔案同理：

- `transport/client.rs` → `transport/client/tests.rs`
- `refactor_assemble.rs` → `refactor_assemble/tests.rs`
- `mechanism/apply.rs` → `mechanism/apply/tests.rs`
- `data/character.rs` → `data/character/tests.rs`

若實作時 Rust module resolution 或既有同名目錄產生衝突，該支先停下來重新複核，不用 `#[path = ...]` 或 `include!` 硬繞；本案優先維持標準 Rust module layout。

## 4. 搬移原則

1. **production body 不動**：除了把 inline test module 換成 `#[cfg(test)] mod tests;`，production code 不改一 byte。
2. **測試不重寫**：不改 test name、assert、expect、測試資料、註解、helper 行為。
3. **module path 不變**：外移後仍是原本的 `...::tests`，讓 private item 與 `super` 路徑照舊可見。
4. **不順手抽 fixture**：即使多支測試看起來重複，本案不建立新的 shared test utility；那是另一個 hygiene 題目。
5. **不改 visibility**：正常情況下不應出現 private → `pub(super)`。若真的需要，視為外移設計有問題，先停工複核。
6. **一次一支主檔**：同一工作段不把 A 級全部搬完。第一輪只做 `transport/assemble.rs`。
7. **一支一 commit**：驗證通過才 commit，下一支從已知綠燈點開始。

## 5. 驗證策略

repo 已有 `scripts/split-verify/test_bodies.py`，會對每支 `#[test]` function body 做 raw hash；這是重要安全網，但這次有一個需要明講的差異：

過去大檔拆分時，test function 通常仍留在某個 inline `mod tests {}` 裡；本案是把 `mod tests` 的**內容升一層成外部 module 檔**。正常 rustfmt 後，測試 body 會少一層固定縮排，因此直接拿舊檔與新 `tests.rs` 跑 raw hash 會把「只少四格縮排」誤判成 body 被改。

本案不因此放棄 byte 級驗證，施工時用兩層檢查：

### 5.1 搬移內容等價

從拆前原檔擷取 `mod tests { ... }` 的內層內容，**只移除固定的一層 module indentation**，再與新 `tests.rs` 做逐 byte 比對。

允許差異只有：

- 外層 `#[cfg(test)] mod tests {` / 最後一個 `}` 被 `mod tests;` 取代；
- 內層內容整體少一層固定縮排。

其他任何文字差異都視為失敗。

### 5.2 測試 body hash

可把拆後 `tests.rs` 生成一份暫存的「重新加回一層縮排」版本，再交給現有 `scripts/split-verify/test_bodies.py` 與拆前原檔比對。如此沿用現有 verifier，不需要為本案修改驗證工具本身。

期望結果：

- 遺失測試 = 0
- 新增測試 = 0
- body 被改 = 0

### 5.3 編譯與測試

每支檔案至少完成：

1. 外移前保存 `cargo test --lib -- --list` 基準。
2. 外移後先跑該 module 的 targeted tests。
3. 跑 `cargo test --lib -- --list`，測試名單與外移前一致。
4. 跑完整 `cargo test --lib`。
5. 跑 `cargo check`。
6. 確認 diff 只有原檔的 test module 宣告變更 + 新測試檔。

`cli/runner.rs` 之後若施工，因為有 Unix / process 相關 cfg，除 host 測試外還要沿用既有 Windows CI 驗證；不要拿 macOS 綠燈當成跨平台完成。

## 6. 分段施工順序

### 工作段 1 — `transport/assemble.rs`

只做這一支：

- 保存拆前基準
- 新增 `src-tauri/src/transport/assemble/tests.rs`
- inline `mod tests { ... }` → `#[cfg(test)] mod tests;`
- 做內容等價比對
- targeted tests + full Rust tests + check
- 驗證乾淨後獨立 commit

這支是最佳 pilot：production 很短、tests 極大、module path 很規則。若這種最單純案例都出現 path / visibility 問題，就先修正本案做法，不往後批次套。

### 工作段 2 — `transport/client.rs`

只有工作段 1 完整綠燈後才做。它的 tests 超過半檔，但牽涉 API/SSE/usage 行為，比 assemble 多一些 async / I/O 測試，適合當第二個驗證外移模式是否穩定的案例。

### 工作段 3 之後

依序：

1. `refactor_assemble.rs`
2. `mechanism/apply.rs`
3. `data/character.rs`
4. 再評估 B 級的 `cli/runner.rs` / `import/export.rs` / `genesis.rs` / `evaluator.rs`

每完成一支都重新看剩餘檔案大小與 test ratio；如果到那時主檔已因其他工程改動，不機械照舊清單施工。

## 7. 明確不做

本案不做：

- production domain 拆分
- test case 合併、刪除、重新命名
- 抽共用 fixture / mock framework
- 改 async test runtime
- 改 `#[cfg]` 條件
- 為了跨檔而提升 private item visibility
- 整理 clippy / rustfmt 以外的既有警告
- 順手修測試發現的功能問題
- 一次把所有 inline tests 全搬掉

若外移過程真的發現既有測試或 production bug，先記錄、停在純搬家邊界，另案處理。

## 8. 完成條件

單支檔案完成的定義：

- production code 僅留下外部 test module 宣告，行為 body 無改動
- 新測試檔 module path 與外移前一致
- test 名單 0 遺失 / 0 新增
- test body（扣除固定 module indentation）0 變更
- targeted tests 綠燈
- `cargo test --lib` 綠燈
- `cargo check` 綠燈
- diff 不含 unrelated cleanup
- 一支一 commit，可獨立 revert

整案不要求一次清空所有 inline tests；完成 A 級後可以重新盤點，再決定 B 級是否值得繼續。這項 hygiene 的目標是讓最吵的大檔先變乾淨，不是追求形式上的 100% 外移率。

## 9. 完成紀錄

完成日期：2026-09-06

本案最後完成 A、B 級共九支 inline test module 外移：

- `transport/assemble.rs`
- `transport/client.rs`
- `refactor_assemble.rs`
- `mechanism/apply.rs`
- `data/character.rs`
- `cli/runner.rs`
- `import/export.rs`
- `genesis.rs`
- `evaluator.rs`

每支 production 檔只留下標準的 `#[cfg(test)] mod tests;`，測試仍位於原本的
`...::tests` module path；沒有為外移提升 production item visibility，也沒有改動 cfg 條件。

最終驗證使用 Cargo 1.97.1、rustfmt 1.9.0。首次完整驗證發現：外移檔仍保留 inline
module 的固定縮排，且 crate 另有既有 rustfmt／Clippy 差異。為讓既定的四項最終 gate
確實全綠，收尾階段執行全 crate rustfmt，並處理 Clippy 指出的等價寫法、重複 attribute
與刻意跨 await 持有的測試序列鎖 lint。這些收尾不改 Tauri command 契約或產品流程。

最終結果：

- `cargo fmt --check`：通過
- `cargo check`：通過
- `cargo test`：536 passed、0 failed、0 ignored
- `cargo clippy --all-targets --all-features -- -D warnings`：通過
- `git diff --check`：通過

完整 Rust 測試包含本機子程序與 loopback mock server；本案沒有前端、資料格式、migration
或真實 AI provider 行為變更，因此不需要另跑 GUI／實機流程。功能分支的分段提交在合併進
`main` 時 squash 成單一提交，讓 repo history 只保留完整工程結果。
