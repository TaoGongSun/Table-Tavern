# Handoff: scene-fork

## Current state
實作完成、四項自驗全綠，等使用者實機驗收後結案。

## Completed
- 資料模型（src-tauri/src/data.rs）：`SceneLabel { base, version, parent, forked }`（data.rs:507）＋`WorldState.scene_labels`（data.rs:494，`#[serde(default)]`，舊存檔照走原線）＋`scene_label()` fallback（data.rs:519：沒進表的幕就是 `base = 幕號, version 1, parent = 幕號-1, forked false`）。
- `fork_scene`（data.rs:2192）：擋掉「不是前面的幕」與空幕後，把源頭幕的事件原樣重寫成新檔，`base` 跟著源頭走、`version` 掃 `0..=current_scene` 數同 base 者 +1、`parent` 指向分岔前所在幕、`forked: true`；`state.state` 取複製內容最後一則的快照。不動 `aligned_scene`。
- `begin_next_scene`（data.rs:2262）加標籤：`base` = 當前幕 base +1、`parent` = 舊幕、`forked: false`。既有行為與 `scene_titles` 存放位置一律未動。
- `revert_scene`（data.rs:2286）改查 `parent`（原線／分岔都適用），退回時連自己那筆 `scene_labels` 一起清掉。
- `replace_scene_summary`（data.rs:2313）與 `regenerate_scene_summary`（lib.rs:1926）同樣改查 `parent`，並加 `forked` 硬擋（見下方拍板）。
- command `fork_scene`（lib.rs:1912，註冊 lib.rs:2161）。
- 前端（src/App.tsx）：`SceneLabel` 型別（App.tsx:186）與 `sceneLabels` 載入（App.tsx:3155、3532）；`sceneDisplayLabel` 改讀標籤表（App.tsx:4752，`version > 1` 才走帶版本號字串，原線行為完全不變）；`forkScene`（App.tsx:4000，先跳 warning confirm 說明成本才動手）；ActReader 的 `onFork` 鈕（App.tsx:3113）＋`.act-fork { margin-left: auto }`（App.css:1795）。
- i18n 十語系五鍵 `sceneFork`／`sceneForkTitle`／`sceneForkConfirm`／`sceneLabelVersioned`／`sceneWithTitleVersioned`。版本號格式一律是各語系既有幕標籤精確加上 `" ({v})"`。

### 拍板：`forked` 標記擋掉會毀資料的一條路
外包回報時發現：分岔的源頭幕若剛好只有一則事件，複製出來的幕也只有一則，前端 `canUndoScene`（`events.length === 1`）與後端「這幕只有一則」的守門**都會放行**——這時按「重寫前情提要」，會把玩家複製過來的真實對話覆寫成摘要。原本「新幕開頭一定是摘要」這個由 `begin_next_scene` 保證的前提，被分岔打破了。
修法是給 `SceneLabel` 加 `forked`，前端不顯示那兩個鈕、後端兩處硬擋（前端守門是 UX，後端才是資料保護）。`revert_scene` 刻意**不擋**分岔幕：它對分岔幕的行為是「刪掉複製出來的幕、回到分岔前」，正確且無害。

## Verification
- `cargo test`：`337 passed; 0 failed`（332 基準＋外包 4 則＋主線補的 `forked` 守門測試 1 則）。`cargo build` 重編 warning 數 = 0。
- 守門回歸測試 `replace_scene_summary_refuses_a_forked_scene`（data.rs:4741）：造出「源頭幕只有一則 → 分岔幕也只有一則」這個守門會誤放的形狀，斷言重寫被拒且那則文字仍是玩家原話。
- 劇本測試 `fork_scene_copies_history_and_relabels_through_continue_and_revert`（data.rs:4900）：涵蓋使用者拍板的驗收例五步（分岔→base 0 v2 parent 2→換幕→base 1 v2→退回落在 parent 而非算術 −1→再分岔得 v3）。
- `npm run build`：✓ 603ms。`npm run check:i18n`：九語系全 OK、84 顆按鈕在寬度上限內。`npm test`：22 passed。
- 未實跑 app：Tauri 原生視窗這邊看不到畫面，實際操作與視覺需使用者確認。

## Remaining / Next action
- 使用者實機驗收：
  1. 開前幕→「從這一幕繼續」→ 跳確認框說明成本，按取消什麼都不發生
  2. 按確認 → 回到對話畫面，內容是那一幕的全部紀錄，幕書籤顯示「第 1 幕 (2)」
  3. 玩一句後按換幕 → 收成「第 1 幕 (2)：（幕名）」，新幕顯示「第 2 幕 (2)」
  4. 前幕清單照時間順序列出，原本三幕內容一字未改
  5. 分岔幕上不出現「重寫前情提要／退回前幕」那兩個鈕
- 不打包：依約 CI 打包等使用者說了才觸發。

## Constraints
同 tasks/scene-fork.md。另注意：`scene - 1` 目前恆等於 `parent`（fork 與換幕都是「新號＝目前號 +1、parent＝目前號」），但四處推算前幕的程式碼已全部改查 `parent`，不要為了省一次查表退回算術寫法——那個不變式沒有任何機制保證。
