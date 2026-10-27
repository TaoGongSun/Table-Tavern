# agy-conversation-isolation — agy 對話寫進使用者自己的 agy 資料夾

## Summary
app 與測試通道開的 agy 對話都寫進使用者的 `~/.gemini/antigravity-cli/`（agy 沒有像 grok、claude 那樣的隔離環境變數）。含私設的角色線凍結 system 因此出現在使用者 agy 的 `/resume` 清單，自動標題也會改述劇情。agy-lane-shape-vs-spec 查證時發現（見 [歸檔交接](../handoffs/archive/agy-lane-shape-vs-spec.md)）。

## Next action
未排程；先查 agy 有沒有可指定資料目錄的方式（設定檔、HOME 覆寫等），沒有的話評估列已知限制或結束時清理。
