# 開場白翻譯：選擇視窗雙鈕（全部翻譯＋翻譯後貼出），走 fast 檔

本檔存放 [opening-translation](../handoffs/archive/opening-translation-completed.md) 的規格細節（拍板結論、分包、驗收等），由任務檔的 Summary 連回。

## 設計要點
1. **兩顆鈕，同一條翻譯呼叫**：視窗上方「全部翻譯」（逐則背景填入、不擋操作，每則有翻譯中狀態）；挑中一則後「翻譯後貼出」與原「貼出」並列（只翻挑中那則）。
2. **一律玩家主動按**（不替玩家花錢紅線）；需要 AI 的按鈕都加 AI 標記，讓玩家知道會用額度。
3. **走 fast 檔**：claude CLI→haiku、codex→low effort、API→tier_models 的 fast 鍵（[cli.rs:336](../../src-tauri/src/cli.rs#L336)）；fast 未設定時退 GM 檔，按鈕永遠可用。
4. **貼出語意照舊**：貼出的譯文就是一般旁白，undo／重匯機制不動；不新增旗標、不撤回重問。預覽清單維持原文。
5. **翻譯提示詞**：開場白內容一律當資料（防注入聲明照重構慣例）；保留 markdown／HTML 標記與內嵌圖片語法，只翻文字；巨集已由 card_openings 換成實名（[lib.rs:381](../../src-tauri/src/lib.rs#L381)），不必處理。
6. **重構管線一字不動**：重構產物本來就是玩家語言，兩者各管各的。
