# Table Tavern — 專案協作規約

- 交接走全域 `handoff` skill（純 Markdown，沒有 CLI）：索引 [.ai/HANDOFF.md](.ai/HANDOFF.md)，一個工作線一個檔 `.ai/handoffs/<id>.md`。交接＝就地改寫該檔成「現在還成立的狀態＋下一步」，不追加歷史、不留已完成的過程；只動自己那條索引，別人的原封不動。
- 開工先整份讀 `.ai/handoffs/<id>.md`；同一時間只做一個工作線。
- commit 以**單一任務**為單位——一案從立案文件、實作到驗收修完，只 commit 該案自己的檔案（含 `.ai/` 文件）一次；中途各輪修正（Sol 或使用者驗收提出的）不單獨 commit，留在工作區。例外：跨天的分包大任務可一包一 commit，但同一包內的驗收修正不另外 commit。新立案的文件**不跟處理中的案子一起帶走**，留在工作區當待辦提醒，等該案自己動工時再一起 commit。commit 前只認自己這案的檔案，不必去檢查工作區其他文件的內容。訊息 `<任務 id>: 做了什麼（驗證結果）`。push 時機由使用者決定。
- 立案一律簡單說明，不深入查詢，不「搞清楚狀況好讓新對話不用查」，新對話開出來就是拿來查的，不要耗費長對話的額度去節省新對話的額度。
- 規格細節（拍板結論、分包、驗收）放 `.ai/plans/<id>.md`，由交接檔連回；交接檔本身只寫還成立的狀態，章節怎麼分自由。
- 待辦清單 [.ai/BACKLOG.md](.ai/BACKLOG.md) 手寫維護，立案說明放 `.ai/tasks/<id>.md`；開工＝把該檔搬進 `.ai/handoffs/`、在 HANDOFF.md 登記一條、BACKLOG 那行刪掉。等實機驗收的排 [.ai/reference/verification-queue.md](.ai/reference/verification-queue.md)。
- [.ai/DONE.md](.ai/DONE.md) 與 `.ai/history/` 是舊 CLI 時代的存檔，凍結只讀。
- **測試卡固定放 `TestCards/`**（repo 根目錄下，已 gitignore、本機限定）：角色卡 `.png`／`.json`、世界書 `main_*_world_info.json`、重構卡 `*-重構卡*.json` 都在裡面。要卡直接去這裡拿，不要全機搜尋。
