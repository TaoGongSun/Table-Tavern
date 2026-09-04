# refactor_ai.rs 拆進 refactor_ai/

狀態：2026-09-04 **拆分施工完成**。原 2764 行 `src-tauri/src/refactor_ai.rs` 已拆成 `src-tauri/src/refactor_ai/`；`legacy.rs` 已刪除；原 56 個測試都有新歸屬。剩餘項目只有在可執行 repo 的環境跑 `npm run build` + `cargo test`，GitHub connector 本身無法執行本地 cargo，因此本文件不冒稱編譯已綠。

## 基準與紅線

原始基準：`src-tauri/src/refactor_ai.rs` blob `9c5cf7a35b33e31cca906728f7b181a193cfaa81`，2764 行（production 1–1810、同檔測試 1811–2764）。切線與依賴以 [refactor-ai-split plan](../plans/refactor-ai-split.md) 為準。

本案沿用 mechanism split：production body 純搬家；只做 module plumbing 必要的 import／`pub(super)`；`mod.rs` 純 facade；不改 `commands/refactor.rs`、`refactor_assemble.rs`、`refactor.rs`；零呼叫端 `RefactorAbsorbOutcome` 留在 `types.rs` 當 `parse_absorb` 回傳型別但不從 facade re-export。

## Production 完成狀態

正式 implementation 共 9 檔：

- `types.rs`
- `context.rs`
- `prompt_common.rs`
- `survey.rs`
- `expand.rs`
- `rewrite.rs`
- `parse_common.rs`
- `survey_parse.rs`
- `result_parse.rs`

`mod.rs` 只做模組宣告、`cfg(test)` 測試宣告與明列 re-export；不再使用 glob facade。

原 34 個 `pub` production item 中，有呼叫端的 33 個仍維持 `refactor_ai::X` 路徑；唯一零呼叫端 `RefactorAbsorbOutcome` 沒有 re-export。facade 重新逐項計數為 `4 + 2 + 5 + 2 + 2 + 3 + 15 = 33`。

型別與使用者同步切 ownership，避免過渡期出現兩套 Rust 型別：`EntryKind + expand`、`GroupKind + rewrite`、`RefactorRecommendOutcome + parse_recommend`、`PrescanSignal + prescan_worldbook + survey_messages`、survey outcome 五型別 + survey parser、result outcome/new-entry 型別 + result parser。

原 `refactor_ai.rs` 曾以同一 Git blob直接 rename 成 `refactor_ai/legacy.rs`，rename diff 為 0 additions / 0 deletions；完成測試搬移後 `legacy.rs` 已刪除。

## 測試 baseline 與最終歸屬

原測試區共有 **56 個 `#[test]`**；test function 名稱維持原名，最終分配：

- `context.rs`：11 個（assemble 1、span 3、format marker 1、prescan 6）。
- `expand.rs`：2 個 prompt tests。
- `rewrite.rs`：4 個 prompt tests。
- `survey_tests.rs`：3 個 survey/recommend prompt tests。
- `survey_parse_tests.rs`：12 個 recommend/survey parser tests。
- `result_parse_tests.rs`：21 個 person/interface/absorb/group/span-placeholder parser tests。
- `types_tests.rs`：2 個 `EntryKind` / `GroupKind` parse tests。
- `integration_tests.rs`：1 個 `all_stage_system_messages_are_byte_identical_for_same_context`。

合計 `11 + 2 + 4 + 3 + 12 + 21 + 2 + 1 = 56`。後四個 owner-specific sibling test files 是為避免 GitHub connector 在純搬家期間整份重寫 200–400 行 production parser；它們只在 `mod.rs` 的 `#[cfg(test)]` 下掛入，不影響 production graph，也沒有回到單一巨大 `tests.rs`。

## 已完成的靜態驗收

- `legacy.rs` 不存在，root 不再宣告 legacy module。
- production implementation 維持原定 9 檔。
- facade 明列 33 個有呼叫端 API。
- `RefactorAbsorbOutcome` 仍存在 `types.rs`，但不在 facade。
- `commands/refactor.rs`、`refactor_assemble.rs`、`refactor.rs` 沒有因拆分改路徑或改碼。
- 56 個原測試都有對應新歸屬；不再依賴 legacy 承載舊 tests。

## 尚需外部可執行驗收

在能執行 repo 的環境依現有 Windows CI 前置順序跑：

1. `npm ci`
2. `npm run build`
3. `cd src-tauri && cargo test`

期望：cargo 全綠，且 refactor_ai 這批 56 個 test name 全部通過。若編譯器只報拆模組造成的 visibility/import 問題，只做最小 plumbing 修正；不要順手改 production 邏輯、prompt 或 parser 容錯。

## 界線

- 不趁拆分收斂重複、改 prompt、改 parser 容錯、改命名或修功能。
- facade 路徑維持 `refactor_ai::X`；不改呼叫端。
- 只推 `chatgpt-collab`；合併 main 由使用者拍板。
