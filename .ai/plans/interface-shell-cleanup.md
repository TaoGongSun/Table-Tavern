# interface-shell-cleanup：清舊產殼路線殘留

來源：[interface-takeover-spike](../handoffs/interface-takeover-spike.md) 待辦 4。舊路線＝重構叫 AI 寫整份自包含 HTML 殼；現行＝重構把卡每回合輸出格式抄成骨架（`{{狀態樹路徑}}`＋`{{本回合.正文}}`），app 填值後過卡自己的顯示腳本。下文「殼」若不另註，都指現行骨架。玩家可見行為不變；唯一例外是舊路線重構過的桌（整頁 HTML 殼），改走近 10 則掃 raw、不做相容〔作者裁決 2026-10-02〕。

## 要改

| # | 位置 | 改法 | 為何安全 |
|---|---|---|---|
| 1 | `src/features/card-interface/useCardInterfaceController.ts:75, 123-150` | 選路整段抽成 #12 的純函式，controller 只接線；刪 `fillShellPlaceholders` 分支與 import；75 行「靜態渲染殼」、125-127 行「整頁 HTML（舊制殼）」註解改成骨架語意。**舊整頁產物辨識**：`refactorShell` trim 後以 `<!DOCTYPE html` 或 `<html` 開頭（不分大小寫、錨定開頭）就當作沒有重構殼，直接走既有近 10 則掃 raw | 光刪分支不夠：舊 HTML 會被當骨架填值送進 `findShell`，而 `extractShell`（`interface-card.ts:146` 的 `BARE_SHELL_MARKER`）在卡腳本沒命中時會把輸入本身的 HTML 抽出來，舊殼照樣顯示。錨定開頭才不會誤殺內含 HTML 片段的合法骨架（骨架照搬卡的 XML 輸出格式，開頭是卡的標籤）。落實作者裁決〔作者裁決 2026-10-02〕，不是相容層 |
| 2 | `src/features/refactor/refactor-shell.ts:1-5, 24-44` | 刪 `escapeHtml`、`fillShellPlaceholders`；檔頭改寫成骨架語意（填值不 escape、餵卡顯示腳本） | 唯一呼叫端就是 #1 那條分支 |
| 3 | `src/features/refactor/refactor-shell.test.ts` | 查路徑、缺值、落在分支、多佔位符、含換行不算佔位符這幾條改測 `fillSkeletonPlaceholders`；escape 與 CSS 花括號那兩條刪掉；補一條「值原文放回、不 escape」 | 查找邏輯共用 `lookupPath`，覆蓋不降 |
| 4 | `src-tauri/src/refactor_ai/parse_common.rs:85-93` | `strip_html_fence` 改名 `strip_code_fence`，註解照實作寫：剝開頭圍欄連同英數語言標記（例 ```xml）；呼叫端 `result_parse.rs:2,88` 跟著改 | 純改名，行為不變 |
| 5 | `src-tauri/src/refactor_ai/result_parse_tests.rs:65-100` | 三條餵 ```html 整頁的測試換成 ```xml 骨架（`<UI>{{World.Time}}</UI>`），`extracts_html_shell` 改名 `extracts_skeleton` | 測的是圍欄剝除與截斷容錯，換輸入不換斷言意義 |
| 6 | `src-tauri/src/refactor/tests/interface.rs:16, 249, 424, 593-605, 713` | HTML／`<div>` fixture 換成 XML 骨架字串；593 的說明註解改掉「HTML」 | fixture 內容與斷言無關（只看落檔、佔位符路徑） |
| 7 | `src/features/refactor/refactor-review.test.ts:93-99, 247-249` | `<p>…</p>` 殼 fixture 換成骨架字串；兩條測試名稱裡的「整份 HTML」「HTML 殼」一併改成骨架 | 同上 |
| 8 | 註解：`src-tauri/src/refactor/types.rs:28-29`、`src-tauri/src/commands/refactor.rs:570-571`、`src-tauri/src/data/paths.rs:69-71`、`src-tauri/src/receipts.rs:24,135-137,393`、`src-tauri/src/refactor_ai/result_parse.rs:77-80`、`src/features/refactor/refactor-review.ts:21,162,217-218` | 「完整 HTML／自包含／靜態渲染殼／下一包串接」改成「卡每回合輸出格式的骨架」 | 只動註解 |
| 9 | `.ai/reference/CARD-REFACTOR-SPEC.md:68,186-206` | 分類處置表：固定資產（11×7 矩陣、座標表）改成「照抄成骨架固定文字，不做狀態欄位」；時間等欄位改成「跟著劇情走的（時間／地點／可用物品／技能／推薦行動）每回合必報，世界層面慢變的變動才報」（照交接檔拍板規格）。渲染三層照實作改寫：①卡片自帶 HTML 殼；②接管桌＝骨架填值過卡顯示腳本，沒命中就退回近 10 則掃 raw；③狀態欄（`table-state/StateBar.tsx` 平欄＋狀態樹）一直都在，跟卡片介面互不相干——**沒有**格子地圖、按鈕列那類通用介面元件，骨架失敗也不會自動切出通用介面。「渲染失敗怎麼辦」那段（AI 產的 HTML 會錯）刪掉 | 規格文件跟現況對齊 |
| 10 | `docs/ARCHITECTURE.md:27` | 「產殼」改「介面骨架」 | 只動文件 |
| 11 | 十語系 `src/i18n/*.ts` 的 `refactorExpanding`、`refactorRewriting` | 刪除 | 全 repo 無引用（refactor-dispatch 留下），`check:i18n` 會驗十語系一致 |
| 12 | 新增 `src/features/card-interface/card-shell-route.ts`＋`.test.ts` | 純函式 `pickCardShell({ tableMode, refactorShell, events, tableTree, cardInterfaces })`，內容就是現在 `cardInterfaceShell` 的 useMemo 本體加 #1 的舊殼辨識 | 純搬移＋一個判斷，hook 難測所以抽出來 |

## 不動〔模型判斷·未裁決〕

- 檔名 `interface-shell.html`、`data::interface_shell_path`／`read_/write_interface_shell`、command `refactor_interface_shell`：現行骨架就存這個檔，已有桌存了它。
- `EntryKind::InterfaceShell`／IPC 字串 `"interface_shell"`、`RefactorInterface.shell`、receipt 的 `interface_shell_created`：序列化進 `refactor-outcome.json`／收據，改名要做遷移，換不到東西。
- `INTERFACE_SHELL_RULES` 與 `prompt_common.rs:73`「不要寫任何 HTML」：內容就是現行骨架契約，那句是防模型退回舊路線的護欄。
- `refactor/interface.rs` 的殼別名正規化、`be_refactor_shell_conflict`（十語系）：骨架一樣有佔位符，衝突檢查仍在用。
- `interface-card.ts` 的 `SHELL_START_MARKERS`／`BARE_SHELL_MARKER`、`buildShellDocument` 與其 HTML 測試：那是卡片自帶的 HTML 殼，跟重構無關。
- 十語系 i18n：舊路線專屬 key 已在 spike 移除 ⓘ 時清掉；另外兩個無人使用的 key 見要改 #11。
- 其他工作線的舊計畫（`plans/refactor-ai-split.md` 等）只是那案的施工紀錄，不改。

## 驗證

- `npm run verify` 全綠（含 check:structure、i18n、vitest、cargo）。
- #12 選路回歸測試（vitest）：舊整頁 HTML 殼退回近 10 則掃 raw（不再顯示舊殼）；合法骨架內含 HTML 片段仍照常過卡腳本；骨架沒過卡腳本退回掃 raw；開場訊息 direct-first（卡腳本直接畫得出來就不塞骨架）；`characters` 模式與 mode 未知（undefined）一律回 null。
- 建議實測：用 `TestCards/WestFantsy.png` 重構接管跑一輪，面板（地圖 11×7、五分頁）照常渲染、時間跟著回合動。
