---
name: tavern-handoff
description: 本專案的工作交接規約。當使用者提到「接手」「交接」「待辦」「上次做到哪」「開工」「收工」「handoff」，或要開始／結束一段開發工作時使用。介面是 repo 內 .ai/ 目錄的 Markdown 檔，git pull 即同步，不需安裝任何工具。
---

# Table Tavern 工作交接規約

兩人協作，彼此接手對方做到一半的工作。所有交接資料都是 repo 裡 `.ai/` 目錄下的 Markdown，git 就是同步機制。

## 開工（接手）流程

1. `git pull` 拿最新交接狀態。
2. 讀 `.ai/TASKS.md`：看「In progress」各任務的一行摘要與下一步。
3. 對要接的任務，開 `.ai/tasks/<task-id>.md`（任務摘要＋下一步＋限制）。
4. 若存在 `.ai/handoffs/<task-id>.md`，整份讀完——`Current state`、`Completed`、`Verification`、`Remaining`、`Next action`、`Constraints` 是上一手留下的完整現場。
5. 從 `Next action` 開工。

## 收工（交棒）流程

收工前把現場留給下一手，缺一不可：

1. **更新 `.ai/handoffs/<task-id>.md>`**（沒有就照現有檔案的格式建一份）：
   - `Current state`：一兩句講清楚現在做到哪。
   - `Completed`：這次新完成的項目（增量補上）。
   - `Verification`：每項完成宣稱附證據——指令輸出摘要或 `檔案:行號`。
   - `Remaining`／`Next action`：下一手打開就能直接動工的具體指示。
2. **同步 `.ai/tasks/<task-id>.md`** 的 `Summary` 與 `Next action`，更新 `Updated` 時間。
3. **同步 `.ai/TASKS.md`** 該任務那一行的「下一步」摘要（保持既有行格式：`- [id](tasks/id.md) — 標題 — 下一步：…`）。
4. **在 `.ai/history/<今天日期 YYYY-MM-DD>.md` 追加一行**：做了什麼、commit hash。
5. `git add .ai && git commit`（訊息含任務 id）並 push。

## 規則

- 「完成／修好」必附證據（指令輸出或檔案:行號）；「找不到」必附搜尋範圍。
- 只編輯 `.ai/` 下的 Markdown。`.json`／`.jsonl` 是某一方本機工具的狀態檔，已被 gitignore，不要建立也不要修改。
- 開新任務：照 `.ai/tasks/` 現有檔案格式建 `tasks/<新-id>.md`，並在 `TASKS.md` 對應區段加一行。任務結案：狀態改 completed、從「In progress」移出，那一行搬到 `.ai/DONE.md`（`TASKS.md` 的 Done 區只留一句連結，不列項目）。
- 同一時間專注一個任務；動工前先 pull，收工必 push，避免兩邊現場分岔。
