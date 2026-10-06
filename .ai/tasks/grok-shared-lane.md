# grok 角色改走共線

Status: todo（等 grok-system-override 修好再開工）〔作者裁決 2026-10-06〕

## Summary
grok 目前一角一線，理由是「session 檔沒有可靠的回合後抹寫路徑」（grok 1.0.5 時代的判斷）。2026-10-06 用 grok 1.0.46 實測：回合後把私設從 session 目錄的 `chat_history.jsonl` 與 `updates.jsonl` 兩個檔一起抹掉，`-r` 續聊時模型答不出私設，抹掉的內容也不會回到任何檔案（`memtrace`／`memory-v2`／logs 都沒有）。只抹 `chat_history.jsonl` 無效——續聊會從 `updates.jsonl` 重建。

改成共線就能像 claude 一樣多角色共用一條快取。風險同 claude：靠的是沒有官方文件的內部檔案格式，CLI 升版可能變；要加抹寫失敗就重開的保護。

## Next action
開工先確認 grok-system-override 修完後的 system 送法，再設計共線的抹寫與失敗保護；沒驗過的有抹寫後的快取命中、app 獨立 grok-home 下的檔案結構。
