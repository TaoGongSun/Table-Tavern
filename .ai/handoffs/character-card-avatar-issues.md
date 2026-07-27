# Handoff: character-card-avatar-issues

## Current state
只登記待辦，尚未動工。2026-07-27 使用者實測 character-image-avatar 結案版後追加五項回饋（原文見下），已逐項對過程式碼定位與成因，寫進 `.ai/tasks/character-card-avatar-issues.md`。

## 使用者原文（2026-07-27）
1. 人物有頭像後，這個頭像和文字太近了，讓他間隔多一點
2. 角色卡不能改名，要有地方改名
3. 移除頭像，編輯人物圖像等沒有被算進未儲存變更
4. 移除頭像沒有出現警告提示，例如「會變回 emoji」
5. 點擊建卡下方會跳出 invalid name: ""，沒有辦法建卡

（附截圖：composer 目標晶片，圓頭像緊貼「Fox」字。）

## Completed
- 無程式修改。僅完成問題定位（見 Verification）與任務登記。

## Verification
- 第 1 項間距：`.opt-target` gap `0.4em`（App.css:1111 起）、`.avatar-round` 外框 `box-shadow: 0 0 0 2px var(--avatar-ring)`（App.css:507）吃掉視覺間距；尺寸三處 App.css:513-515。
- 第 2 項改名：全庫 grep `rename` 只有 `rename_world`（data.rs:407、lib.rs:198、指令註冊 lib.rs:946），沒有角色改名路徑；桌名就地編輯 UI 在 App.tsx:2602-2614。
- 第 3 項未儲存：`CharacterCardEditor`（App.tsx:1449 起）無任何 dirty／unsaved 狀態；世界設定的對照實作在 App.tsx:967、1135。`removeImage` App.tsx:1597、`removeAvatar` App.tsx:1607 皆為立即 invoke 寫檔。
- 第 4 項警告：`removeAvatar`（App.tsx:1607）無 `confirm`，直接 `delete_character_avatar`。
- 第 5 項空名：`createCharacter`（App.tsx:2184-2191）只擋 GM／玩家；後端 `validate_name`（data.rs:197-208）對空字串回 `invalid name: ""`。

## Remaining
五項全部。建議順序：5 → 4 → 1 → 3 → 2。

## Next action
先向使用者拍板兩件事：(a) 第 3 項要「圖像操作改暫存、按儲存才生效」還是「圖像維持立即生效、只補文字欄位的未儲存提示」；(b) 第 2 項改名是否要回填既有場景紀錄／世界書裡的角色名引用。拍板後從第 5 項（擋空名＋建卡鈕 disabled＋i18n 錯誤字串）開工。

## Constraints
- 沿用 character-image-avatar 既有拍板：頭像存正方形 PNG、圓框走 CSS；移除全身圖不連動刪頭像；刪角色清兩檔。
- 改名須沿用 `validate_name` 規則並擋同名碰撞（比照 data.rs:1897 `renames_world_and_rejects_collisions`）。
