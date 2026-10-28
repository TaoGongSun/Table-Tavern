# claude-bare-default — claude 官方預告 `--bare` 將成 `-p` 預設，bare 不讀訂閱登入

## Summary
2026-10-28 cli-version-gate 查官方文件發現：claude 的 `--bare` 未來會變成 `-p`（無頭）的預設，而 bare 模式不讀訂閱登入。屆時 app 的 claude 路徑每輪都會回登入錯誤。

## Next action
未排程。先查官方文件目前的說法與時程、有無「不要 bare」的明確旗標；確認後再決定 app 呼叫要不要先明帶對應旗標。目標是玩家不必更新也能繼續用。
