# refactor_ai.rs 拆進 refactor_ai/

狀態：2026-09-03 20:20 階段一（依賴複核）已發給 ChatGPT，**落地未驗證，主線額度觸頂中斷**。

## 現況
`src-tauri/src/refactor_ai.rs` 2764 行（本體 1–1810／同檔測試 1811–2764），mechanism-split 結案後的最大檔（按本體行數）。做法整套沿用 [mechanism-split](../plans/mechanism-split.md)：純搬家、production body 逐 byte 不動、`mod.rs` 只當 facade、零呼叫端的 re-export 不掛。

本體約 90 個頂層 item，粗數 34 個對外 `pub`（含 `assemble_card_context`、`segment_spans`、`prescan_worldbook`、`survey_messages`、`recommend_messages`、`parse_survey`、`parse_expand`、`parse_absorb`、`parse_group`、`expand_span_placeholders` 等）。精確盤點是階段一的產出，不在本檔重抄。

## 走到哪
走 `claude-with-chatgpt` 技能，分四階段（階段一依賴複核／階段二搬底層／階段三搬上層加 facade／階段四搬測試）。

階段一訊息 2026-09-03 19:57 送出到 ChatGPT「Table Tavern」專案新對話
`https://chatgpt.com/g/g-p-6a5b72c4393c8191964917fa15e41b82-table-tavern/c/6a995fe2-58fc-83e8-8ed6-457a25293292`

交辦內容：只做依賴複核不改 .rs，四件事（頂層 item 盤點／依賴 DAG 與有無環／切線定案含共用型別放哪／pub item 對外呼叫端清查分「有呼叫端」「零呼叫端」），全部寫進新檔 `.ai/plans/refactor-ai-split.md` commit 到 `chatgpt-collab`；不准動 `src-tauri/`、不准開 PR、不准開 Codex 任務。

20:20 查證：`chatgpt-collab` HEAD 仍是 `cc8c65e`（上一案），**新檔未落地**。送出至今 22 分鐘，接近網頁版 25 分鐘砍斷線。主線 5 小時額度 98% 觸頂，瀏覽器工具回 429，無法再看畫面。額度重設 2026-09-03 22:50（台灣時間）。

## 下一步
1. 額度回來後先 `gh api repos/TaoGongSun/Table-Tavern/branches/chatgpt-collab --jq '.commit.sha'` 查有沒有新 commit（GitHub 讀取有快取延遲，查不到先等 60–90 秒重查一次）。
2. 有 commit → `git fetch origin chatgpt-collab`，`git show FETCH_HEAD:.ai/plans/refactor-ai-split.md` 唯讀比對；確認只多這一個檔、`src-tauri/` 零變動，再進階段二。
3. 沒有 commit → 回原對話看是不是被 25 分鐘砍斷；被砍就把階段一再切小（例如先只做「頂層 item 盤點＋pub 呼叫端清查」，DAG 與切線定案另一輪）。

## 界線
- 純搬家：不趁本案收斂重複、拆函式、改命名或改邏輯。
- facade 保住拆前所有**有呼叫端**的 `refactor_ai::X` 路徑與可見度；零呼叫端者不留。
- 可見度只做編譯器逼出來的最小放寬（`pub(super)` 優先）。
- 只推協作分支，合併回 main 要使用者拍板。

## 待處理的文件不一致（非本案）
`.ai/handoffs/mechanism-split.md` 與 `.ai/HANDOFF.md` 都還寫 mechanism-split「未開工」，但 `src-tauri/src/mechanism/` 已 10 檔、commit `cc8c65e` 就是完成那筆。等使用者拍板要不要改。
