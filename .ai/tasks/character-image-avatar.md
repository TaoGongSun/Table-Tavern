# Task
Task-ID: character-image-avatar
Title: 角色圖片管理：app 內加入／更換／移除全身圖＋圓形頭像裁切取代 emoji
Status: in_progress
Created: 2026-07-27T00:00:00+08:00
Updated: 2026-07-27T14:45:00+08:00

## Summary
現況只有匯入角色卡能帶圖。2026-07-27 拍板：角色編輯頁可加入／更換／移除全身圖（react-easy-crop 矩形裁切、鎖 2:3 直式）；從全身圖再裁圓形頭像（存正方形 PNG，CSS 圓形＋黑框 `--avatar-ring`）取代 emoji，可移除換回 emoji；頭像一律從該角色唯一全身圖裁出、不另選圖（對齊未來 AI 生圖：一角色一本源圖）。已匯入角色走同一入口。檔名：全身圖沿用 `characters/<name>.png`，頭像 `<name>.avatar.png`。

## Next action
- 使用者實測（commit fa4d855）：加圖→裁切→顯示、製作頭像→列表與發言者列圓頭像黑框、移除頭像回 emoji、更換／移除圖片、已匯入角色重裁。通過即結案

## Constraints
- 頭像存正方形 PNG（256×256），圓形與黑框由 CSS 畫（border-radius:50% + box-shadow ring），不烘進圖檔
- 全身圖裁切鎖 2:3 直式、輸出上限寬 1024；覆寫 `<name>.png`（匯入卡的內嵌 metadata 已抽進 md，覆寫可接受）
- 移除全身圖不連動刪頭像（各自獨立檔）；刪角色兩檔都清
- emoji 欄位保留當 fallback
