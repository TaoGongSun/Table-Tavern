# Table Tavern — 專案協作規約

- 任務與交接走全域 `maintaining-task-handoffs` skill：待辦索引 [.ai/TASKS.md](.ai/TASKS.md) 與任務檔 `.ai/tasks/` 一律以 `handoff task …` 指令異動，禁止手改；長任務交接用 `handoff checkpoint／pause／complete`。
- 開工先整份讀 `.ai/handoffs/<task-id>.md`；同一時間只做一個 active 任務。
- commit 以功能完成（實作＋自驗綠）為單位，訊息 `<任務 id>: 做了什麼（驗證結果）`；只動 `.ai/` 的文件變更不單獨 commit，push 時機由使用者決定。
- 任務的規格細節（拍板結論、分包、驗收）放 `.ai/plans/<task-id>.md`，由任務檔 Summary 連回——CLI 寫回任務檔時只保留 Summary／Progress／Next action／Constraints 四段，其餘章節會被抹掉。
- `.ai/` 下的 `.json`／`.jsonl` 是 CLI 本機狀態（已 gitignore），只透過指令異動，勿手改。
- [.ai/DONE.md](.ai/DONE.md) 為 2026-08-13 以前的結案清單，凍結不再增修；此後結案由 `handoff task complete` 寫入 `.ai/history/`。
- **測試卡固定放 `TestCards/`**（repo 根目錄下，已 gitignore、本機限定）：角色卡 `.png`／`.json`、世界書 `main_*_world_info.json`、重構卡 `*-重構卡*.json` 都在裡面。要卡直接去這裡拿，不要全機搜尋。
