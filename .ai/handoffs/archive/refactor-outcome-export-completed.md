# Handoff: refactor-outcome-export

## Current state
**實作完成、主線四項驗證全綠**（2026-08-10）。剩使用者實機操作：orc-cave 桌真跑 B1→結果卡「匯出」存真產物→接 [ai-card-refactor](../ai-card-refactor.md) A 段實測（A4／A7 驗本功能）。

## Spec（發包規格）
後端（src-tauri/src/）：
- data.rs：`write_refactor_outcome`／`read_refactor_outcome`（照 `interface_shell` 同款 helper，檔名 `worlds/<id>/refactor-outcome.json`）。
- refactor.rs `apply()` 成功尾端把收到的 outcome `to_string_pretty` 落檔；undo 與收據不動檔（零改動）。
- lib.rs 新 command：`refactor_export_outcome(outcome, path)`（結果卡用，直接寫使用者選的 path）；`refactor_export_saved(world_id, path)`（世界書工具列用，無檔回 `Err("refactor-export-none")`）。
- cargo 測試：apply 落檔 round-trip 讀回、export_saved 無檔 Err。

前端（src/App.tsx）：
- 結果卡摘要頁 footer「不要」「展開細看」之間加「匯出」鈕：`saveDialog`（defaultPath `refactor-outcome.json`、json filter）→ invoke → `revealItemInDir`（照 exportCard 慣例）。
- 世界書工具列「匯入重構卡」後加「匯出重構卡」鈕：同流程；錯誤含 `refactor-export-none` → `t("refactorExportNone")` 進 worldbookMessage。
- i18n 十語系 +4 鍵：`refactorExportBtn`（匯出）／`refactorExportSavedBtn`（匯出重構卡）／`refactorExportNone`（這桌還沒有重構卡）／`refactorOutcomeJson`（重構卡 JSON，存檔 filter 名）。玩家面用詞 2026-08-10 拍板統一「重構卡」，既有匯入鈕與相關訊息十語系同步改名。
- vitest 不加（無新純函式）。

## Verification
主線親跑（2026-08-10）：cargo test **428 全綠**（基線 426＋新增 2：`apply_writes_refactor_outcome_file_readable_and_round_trips`、`apply_then_undo_keeps_refactor_outcome_file`）／vitest 82／npm build／check:i18n 十語系 OK（de×2、es、fr、ja、pt-BR、ru 共七處匯出鈕譯文超寬，主線修短後全過）。主線審過後端全部 diff（data.rs helper 照殼檔慣例、兩 command 已註冊、apply 落檔在全部套用動作之後）與 App.tsx 兩顆鈕接線；契約型別補 `PartialEq` derive 供 round-trip 測試。

已知限制：apply 尾端寫產物檔若失敗（磁碟錯誤等罕見情況），套用其實已完成但玩家會看到錯誤——與既有「中途 Err 已落檔無收據可退」同型，不阻塞。

## Next action
主線驗證四項全綠→commit→使用者拿實際測試卡（orc-cave）真跑 B1→結果卡「匯出」存 TestCards/→接 [ai-card-refactor](../ai-card-refactor.md) A 段真產物實測。
