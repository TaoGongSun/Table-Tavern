# Task
Task-ID: character-card-avatar-issues
Title: 角色卡回饋修訂：頭像間距／角色改名／圖像變更未儲存提示／建卡空名擋下
Status: todo
Created: 2026-07-27T00:00:00+08:00
Updated: 2026-07-27T00:00:00+08:00

## Summary
character-image-avatar 結案後使用者實測回報的五項（2026-07-27 追加）：

1. **頭像與文字太近**——有圖頭像的晶片／卡片，圓頭像和名字幾乎貼著（截圖為 composer 目標晶片「Fox」）。原因：`.opt-target { gap: 0.4em }`（App.css:1111 起）沒把 `.avatar-round` 的 2px `box-shadow` 外框算進去，視覺間距又被吃掉 2px；晶片左內距 `padding: 0 0.9em` 同樣不夠。三處都要對（`.opt-avatar` App.css:514、`.tcard-avatar` :513、編輯頁 `.card-editor-avatar-round` :515）。
2. **角色卡不能改名**——沒有任何改名入口。後端只有 `rename_world`（data.rs:407、lib.rs:198），沒有 `rename_character`。桌名改名的互動可比照（App.tsx:2602 起，點名字就地編輯＋`renameHint`）。要一併處理的連動：`characters/<name>.md`、`<name>.png`、`<name>.avatar.png`、生成圖庫目錄，以及既有場景紀錄／世界書條目裡以名字為索引的引用（改名後是否回填歷史，需拍板）。
3. **移除頭像、編輯角色圖像不算進未儲存變更**——`CharacterCardEditor` 完全沒有未儲存追蹤（對照世界設定 App.tsx:967 `unsavedCount` 與 App.tsx:1135 的提示列）。另注意 `removeImage`／`removeAvatar`（App.tsx:1597、1607）是**立即寫檔**，本來就沒有「未儲存」狀態；所以這項要先拍板方向：(a) 圖像操作改成暫存、按儲存才落地並計入未儲存變更；還是 (b) 圖像維持立即生效，但角色卡文字欄位補上未儲存計數＋離開確認。
4. **移除頭像沒有警告**——`removeAvatar`（App.tsx:1607）直接刪檔，沒有 `confirm`。要比照刪角色的確認框，文案講清楚後果（「會變回 emoji 顯示」）。移除全身圖是否比照，一起拍板（移除全身圖不連動刪頭像，是既有拍板）。
5. **建卡空名炸錯誤**——`createCharacter`（App.tsx:2184）只擋保留名 GM／玩家，沒擋空字串，空白直接送 `write_character`，被後端 `validate_name`（data.rs:197）打回 `invalid name: ""`，使用者看到生錯誤訊息且建不了卡。前端要先擋空名（並讓建卡鈕在空名時 disabled），錯誤訊息走 i18n；順帶檢查其他 `validate_name` 會擋的字元（`/`、`\`、開頭 `.`、`..`）是否也該前端先提示。

優先序建議：5（擋住建卡的實錯）→ 4（資料誤刪風險）→ 1（純 CSS）→ 3 →2（需後端新指令＋改名連動拍板）。

## Next action
- 先拍板第 3 項方向（圖像操作暫存 vs 立即生效）與第 2 項改名的歷史引用連動範圍，其餘四項可直接動工，從第 5 項開始

## Constraints
- 頭像仍存 256×256 正方形 PNG，圓形與黑框由 CSS `--avatar-ring` 畫，不烘進圖檔（沿用 character-image-avatar 拍板）
- 移除全身圖不連動刪頭像；刪角色兩檔都清
- 改名若動到檔名，須沿用 `validate_name` 規則並擋同名碰撞（比照 `rename_world` 的碰撞測試 data.rs:1897）
