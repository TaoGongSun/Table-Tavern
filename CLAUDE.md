# Table Tavern — 專案協作規約

- 交接走全域 `handoff` skill（純 Markdown，沒有 CLI）：索引 [.ai/HANDOFF.md](.ai/HANDOFF.md)，一個工作線一個檔 `.ai/handoffs/<id>.md`。交接＝就地改寫該檔成「現在還成立的狀態＋下一步」，不追加歷史、不留已完成的過程；只動自己那條索引，別人的原封不動。
- 開工先整份讀 `.ai/handoffs/<id>.md`；同一時間只做一個工作線。
- **一案一分支**：立案文件寫完就開 `<任務 id>` 分支並 push——ChatGPT 網頁版靠 GitHub connector 讀 repo，沒 push 就看不到。整案在該分支做到結案，分支上隨時可 commit（結案會壓掉）。
- **結案才進 main**：先確定 ChatGPT 那邊已收工（改寫歷史要 force push，它手上還拿著舊的一份會打架），再整理分支歷史——一般案 squash 成一筆，分包大案整理成一階段一筆、中途的修正 commit 併進它所修的那一階段——然後合併進 main、push、把分支的本地與遠端都刪掉。
- 進 main 的 commit 只認自己這案的檔案（含 `.ai/` 文件），不必去檢查工作區其他文件的內容。訊息 `<任務 id>: 做了什麼（驗證結果）`。
- 立案一律簡單說明，不深入查詢，不「搞清楚狀況好讓新對話不用查」，新對話開出來就是拿來查的，不要耗費長對話的額度去節省新對話的額度。
- 規格細節（拍板結論、分包、驗收）放 `.ai/plans/<id>.md`，由交接檔連回；交接檔本身只寫還成立的狀態，章節怎麼分自由。
- 待辦清單 [.ai/BACKLOG.md](.ai/BACKLOG.md) 手寫維護，立案說明放 `.ai/tasks/<id>.md`；開工＝把該檔搬進 `.ai/handoffs/`、在 HANDOFF.md 登記一條、BACKLOG 那行刪掉。等實機驗收的排 [.ai/reference/verification-queue.md](.ai/reference/verification-queue.md)。
- [.ai/DONE.md](.ai/DONE.md) 與 `.ai/history/` 是舊 CLI 時代的存檔，凍結只讀。
- **測試卡固定放 `TestCards/`**（repo 根目錄下，已 gitignore、本機限定）：角色卡 `.png`／`.json`、世界書 `main_*_world_info.json`、重構卡 `*-重構卡*.json` 都在裡面。要卡直接去這裡拿，不要全機搜尋。
- **檔案盡量不超過 1000 行**：不是死規則，必須超過時就超過，但預設往「拆成適當小檔」的方向做——依功能拆，不是為了行數硬切。同理，一個資料夾檔案多到掃不完（例如超過 20 個）就考慮開子資料夾分類。都是方向，看狀況判斷。
