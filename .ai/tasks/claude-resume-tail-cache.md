# claude 續聊每輪重寫對話尾端

Status: todo

## Summary
claude-1h-cache 實測（2026-10-07，Sonnet）：CLI `--resume` 續聊只重用 system 段的快取，前一輪對話尾端每輪重寫（範例桌第二、三輪都只讀 527／1609；直接 CLI 開線＋兩次 resume，帶不帶 1h 環境變數都讀約 7840、尾端每輪重寫）。命中率因此到不了九成，範例桌額度分頁亮「該中沒中」；釘 1h 後寫入是 2 倍價，這段重寫比 5m 時貴。材料在 claude-1h-cache 計畫「四、驗收」。作者 2026-10-07 同意立案、排進待辦。

## Next action
未排程。開工先查 CLI resume 時快取斷點放在哪（session 檔重組、抹寫是否改動了尾端內容、CLI 本身行為），判斷 app 這邊能不能讓前幾輪對話也命中。
