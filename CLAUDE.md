# Table Tavern — 專案協作規約

- 任務與交接走全域 `maintaining-task-handoffs` skill：待辦索引 [.ai/TASKS.md](.ai/TASKS.md) 與任務檔 `.ai/tasks/` 一律以 `handoff task …` 指令異動，禁止手改；長任務交接用 `handoff checkpoint／pause／complete`。
- 開工先整份讀 `.ai/handoffs/<task-id>.md`；同一時間只做一個 active 任務。
- commit 以**單一任務**為單位——一案從立案文件、實作到驗收修完，只 commit 該案自己的檔案（含 `.ai/` 文件）一次；中途各輪修正（Sol 或使用者驗收提出的）不單獨 commit，留在工作區。例外：跨天的分包大任務可一包一 commit，但同一包內的驗收修正不另外 commit。新立案的文件**不跟處理中的案子一起帶走**，留在工作區當待辦提醒，等該案自己動工時再一起 commit。commit 前只認自己這案的檔案，不必去檢查工作區其他文件的內容。訊息 `<任務 id>: 做了什麼（驗證結果）`。push 時機由使用者決定。
- 立案一律簡單說明，不深入查詢，不「搞清楚狀況好讓新對話不用查」，新對話開出來就是拿來查的，不要耗費長對話的額度去節省新對話的額度。
- 任務的規格細節（拍板結論、分包、驗收）放 `.ai/plans/<task-id>.md`，由任務檔 Summary 連回——CLI 寫回任務檔時只保留 Summary／Progress／Next action／Constraints 四段，其餘章節會被抹掉。
- `.ai/` 下的 `.json`／`.jsonl` 是 CLI 本機狀態（已 gitignore），只透過指令異動，勿手改。
- [.ai/DONE.md](.ai/DONE.md) 為 2026-08-13 以前的結案清單，凍結不再增修；此後結案由 `handoff task complete` 寫入 `.ai/history/`。
- **測試卡固定放 `TestCards/`**（repo 根目錄下，已 gitignore、本機限定）：角色卡 `.png`／`.json`、世界書 `main_*_world_info.json`、重構卡 `*-重構卡*.json` 都在裡面。要卡直接去這裡拿，不要全機搜尋。
