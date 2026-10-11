# 換／刪角色圖會蓋掉匯入的原卡 PNG

Status: todo

## Summary
卡片編輯器換角色圖（`save_character_image`）寫到 `characters/<id>.png`，刪圖（`delete_character_image`）刪同一檔（`import/images.rs:56-67`、`:102-130`）；匯入的 PNG 原卡也存在這個路徑。玩家換一次圖，原卡內嵌的卡片資料（介面腳本、世界書等）就永久消失，卡片介面面板跟著不見，重構也改判成沒有介面。屬資料遺失。根治方向：原卡與顯示圖分開存。refactor-silent-skips 審查時發現。

## Next action
未排程；開工先確認現行哪些讀取點把 `<id>.png` 當原卡、哪些當顯示圖。
