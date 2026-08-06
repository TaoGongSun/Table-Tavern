# Handoff: scene-revert

## Current state
實作完成、四項自驗全綠，等使用者實機驗收後結案。

## Completed
- 後端兩個資料層函式（src-tauri/src/data.rs）：
  - `revert_scene`（data.rs:2190）：擋掉 `scene == 0` 與「這幕不只一則」後，刪掉該幕 jsonl、`current_scene` -1、撤掉換幕時存的幕名、`state.state` 回推成前幕最後一則的快照。擋下時故意先讀完才判斷，錯誤路徑不留任何副作用。刻意不動 `aligned_scene`（退回後 current_scene 落回前幕，前幕本來就對齊過）。
  - `replace_scene_summary`（data.rs:2216）：只換那則的 text 與 ts，**其餘欄位原樣保留**——尤其 `state` 快照，見下方拍板。
  - `format_scene_summary`（data.rs:2141）：語系前綴抽出共用，`begin_next_scene` 改呼叫它，兩處不各自維護一份。
- 後端兩個 command（src-tauri/src/lib.rs）：`revert_scene`（lib.rs:1905）、`regenerate_scene_summary`（lib.rs:1913，結構照 `advance_scene`，摘要對象換成前一幕；模型呼叫前先做完兩道守門檢查，不白花一次呼叫）。註冊於 lib.rs:2153-2154。
- 前端（src/App.tsx）：`revertScene`／`regenerateSummary`（App.tsx:3980-4014）；守門 `canUndoScene = scene > 0 && events.length === 1 && generating === null`（App.tsx:4700）；鈕列複用既有 `.undo-restore`，CSS 只加 `gap: 0.5rem`（App.css:1783）；JSX 在復原鈕之後（App.tsx:5441）。退回刻意不呼叫 `noteTurnDone`——退回不是推進，且 lane 一定會因場號對不上重開，保溫已無意義。
- i18n 十語系四鍵 `sceneRevert`／`sceneRevertHint`／`sceneSummaryRetry`／`sceneSummaryRetryHint`，緊接 `sceneAdvanceHint` 之後。費用提示（「會再花一次呼叫」）每個語系都帶到。de 的 `sceneSummaryRetry` 初稿超過按鈕寬度上限，改用「Neu zusammenfassen」。

### 拍板：重寫摘要只換文字，狀態快照留著
外包初版照規格造了一則全新事件（`state: None`），但 `begin_next_scene` 那則是走 `append_transcript` 落地、會被自動補上當時快照。差異在「重寫摘要→再換下一幕→退回這一幕」時會咬到：那則摘要是回推狀態的唯一來源，快照掉了 `revert_scene` 就只能 `unwrap_or_default()`，狀態欄被清空。改成讀出原事件、只換 text／ts，語意也更正確（重寫的是文字，不是狀態），程式碼還更短。

## Verification
- `cargo test`：`332 passed; 0 failed`（327 基準＋外包 4 則＋主線補的回歸測試 1 則）。
- 回歸測試 `replace_scene_summary_keeps_snapshot_for_later_revert`（data.rs:4717）：跑完整路徑「第 0 幕留快照→換幕→重寫摘要→再換一幕→退回第 1 幕」，斷言 `state.state` 仍等於原快照。修正前這條必紅（會落到 `unwrap_or_default()`）。
- `npm run build`：✓ built in 565ms（tsc 綠）。
- `npm run check:i18n`：九個非正典語系全 OK，83 顆按鈕都在寬度上限內。
- `npm test`：Tests 22 passed（3 檔）。基準確實是 22——card-import-flow 結案時刪掉 companion 路徑連帶減一則，早兩個 commit 訊息裡的「23」是當時的數字。
- 未實跑 app：Tauri 原生視窗這邊看不到畫面，UI 位置與手感需使用者實機確認。

## Remaining / Next action
- 使用者實機驗收六項：換幕後兩個鈕出現在訊息串底部；按「重寫前情提要」換到新文字且幕名跟著換；按「退回前幕」回到前幕全部紀錄可續玩、狀態欄數值沒被清空；發一句話後兩個鈕消失；生成中兩個鈕不可按；第一幕（scene 0）不出現這兩個鈕。
- 不打包：依約 CI 打包等使用者說了才觸發。

## Constraints
同 tasks/scene-revert.md。另注意：新幕已經玩了幾句才想退回，正解是先用既有「收回上一句」連按收到只剩摘要，退回鈕自然重新出現——兩個功能天然接得起來，不要為此放寬守門條件。
