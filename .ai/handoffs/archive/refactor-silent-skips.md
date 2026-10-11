> 結案 2026-10-12：進 main，verify 12 步綠、三方驗收 PASS。

# 重構路徑其餘靜默略過

Status: done

## Summary
原卡檔在但讀不出（IO 錯誤、PNG 結構壞、卡片資料解不開）不再被當成「沒有介面」：`card_interfaces` 回一筆 `unreadable`，重構最優先擋下並請玩家開新桌重新匯入；一般 PNG（換過角色圖）不算損毀、改讀 `.import.json`。重構脈絡讀不到既有角色改回錯並帶角色名。取消重構的 abort 失敗時，限原桌原段進度還原取消鈕並報原因。規格與已知限制見 [plans/refactor-silent-skips.md](../../plans/refactor-silent-skips.md)。
