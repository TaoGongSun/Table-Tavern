# 刪檔遇到查詢錯誤誤報成功

Status: todo

## Summary
`remove_path_raw` 用 `path.exists()` 判斷，父目錄沒有搜尋權限等查詢錯誤時回 false，被當成「檔案不在」而回成功，實際沒刪。character-delete-visibility-cleanup 之後，刪角色在這情況回成功＋`worldbook_cleanup_failed`，前端當成已刪但卡還在。

## Next action
未排程。改用 `Path::try_exists`，查詢錯誤回 Err；補拿掉 `characters/` 搜尋權限的測試。
