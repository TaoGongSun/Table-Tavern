# refactor_ai.rs 拆進 refactor_ai/

狀態：2026-09-04 production 拆分已接線完成；舊檔只在 `#[cfg(test)]` 下暫存。測試 baseline 已盤點，context/expand/rewrite 共 17 個測試已搬，下一階段繼續搬其餘測試與做可執行驗收。

## 基準與紅線

原始基準：`src-tauri/src/refactor_ai.rs` blob `9c5cf7a35b33e31cca906728f7b181a193cfaa81`，2764 行（production 1–1810、同檔測試 1811–2764）。切線與依賴以 [refactor-ai-split plan](../plans/refactor-ai-split.md) 為準。

本案沿用 mechanism split：production body 純搬家；只做 module plumbing 必要的 import／`pub(super)`；`mod.rs` 純 facade；不改 `commands/refactor.rs`、`refactor_assemble.rs`、`refactor.rs`；零呼叫端 `RefactorAbsorbOutcome` 留在 `types.rs` 當回傳型別但不從 facade re-export。

## Production 已完成

- 底層：`types.rs`、`parse_common.rs`、`prompt_common.rs`。
- 上層：`context.rs`、`survey.rs`、`expand.rs`、`rewrite.rs`、`survey_parse.rs`、`result_parse.rs`。
- `mod.rs` 已改成明列 facade，不再使用 `pub use legacy::*`。
- 33 個有呼叫端的原 `pub` item 已由新模組提供；`RefactorAbsorbOutcome` 沒有 facade re-export。
- 型別與使用者一起切 ownership，避免過渡期出現兩套 Rust 型別：
  - `EntryKind` + expand
  - `GroupKind` + rewrite
  - `RefactorRecommendOutcome` + `parse_recommend`
  - prescan `PrescanSignal` + `prescan_worldbook` + `survey_messages`
  - survey outcome 五型別 + `parse_survey`/`normalize_survey_for_mode`
  - result outcome/new-entry 型別 + result parsers
- 原 `refactor_ai.rs` 曾以同一 Git blob直接 rename 成 `refactor_ai/legacy.rs`；該搬名 diff 為 0 additions / 0 deletions。
- 現在 `mod.rs` 對 `legacy` 使用 `#[cfg(test)] mod legacy;`：production build 不再編譯舊 implementation；測試模式暫時保留原同檔 tests。

## 測試 baseline

原 `legacy.rs` 測試區 1811–2764 共 **56 個 `#[test]`**，名稱與 owner 分配如下：

- `context.rs`：11 個。assemble 1、span 3、format marker 1、prescan 6。**已搬**，測試名稱不改。
- `survey.rs`：3 個。`recommend_messages_share_survey_system_byte_identical`、`survey_messages_carry_mode_specific_user_instructions`、`survey_messages_injects_prescan_signals_into_user_message`。
- `survey_parse.rs`：12 個。`recommend_parses_two_lines_and_rejects_garbage`、MODE echo、六區塊完整解析、normalize、舊格式、playable default、single-source person、chitchat、三種 malformed line、malformed groups。
- `result_parse.rs`：21 個。person parser 4、interface parser 8、absorb parser 3、group parser 3、span placeholder 3。
- `expand.rs`：2 個。shell prompt、no-spoiler/known-fields prompt。**已搬**，測試名稱不改。
- `rewrite.rs`：4 個。absorb prompt 1、group prompt 3。**已搬**，測試名稱不改。
- `types.rs`：2 個。`EntryKind` / `GroupKind` parse。
- root 小型 integration：1 個。`all_stage_system_messages_are_byte_identical_for_same_context`。

合計 `11 + 3 + 12 + 21 + 2 + 4 + 2 + 1 = 56`；目前 **17 / 56 已搬，剩 39**。

legacy 尚未刪，所以 test build 暫時會同時看到舊測試與新測試，總數暫時膨脹。這是施工中狀態，不能拿目前總數當驗收值。最終刪 legacy 後，refactor_ai 這批 test-name multiset 應回到上述 56 個，名稱逐一相同。

## 尚未完成

1. 依上面的 owner 清單搬剩餘 **39 個**測試；只搬，不改 test body / test name。
2. 搬完後刪除 `legacy.rs` 與 `#[cfg(test)] mod legacy;`。
3. 驗收：refactor_ai 測試名稱 multiset = baseline 56；production body / public API 與計畫一致；facade 無零呼叫端 re-export。
4. 在可執行 repo 的環境跑 `npm run build` + `cargo test`；GitHub connector 本身不能直接執行本地 cargo，因此目前不能宣稱編譯已綠。

## 下一工作段

先搬 `survey.rs` 3 個 + `survey_parse.rs` 12 個；再搬 `result_parse.rs` 21 個。最後補 types 2 個與 root integration 1 個，刪 legacy 後做 56-test 名稱 multiset 驗收。不要再動 production 邏輯。

## 界線

- 不趁拆分收斂重複、改 prompt、改 parser 容錯、改命名或修功能。
- facade 路徑維持 `refactor_ai::X`；不改呼叫端。
- 只推 `chatgpt-collab`；合併 main 由使用者拍板。
