# Handoff: character-card-avatar-issues

## Current state
第 1 項（頭像與文字間距）已修完待實測，其餘四項未動工。2026-07-27 使用者拍板兩件事：第 3 項圖像操作**改成暫存、按儲存才落地並計入未儲存計數**；第 2 項改名**要回填**既有場景紀錄／世界書裡的角色名引用。動工順序改為 1 ✅ → 5 → 4 → 3 → 2。

## 使用者原文（2026-07-27）
1. 人物有頭像後，這個頭像和文字太近了，讓他間隔多一點
2. 角色卡不能改名，要有地方改名
3. 移除頭像，編輯人物圖像等沒有被算進未儲存變更
4. 移除頭像沒有出現警告提示，例如「會變回 emoji」
5. 點擊建卡下方會跳出 invalid name: ""，沒有辦法建卡

（附截圖：composer 目標晶片，圓頭像緊貼「Fox」字。）

## Completed
- 問題定位與任務登記（見 Verification）。
- **第 1 項間距修正**（App.css 四處，純 CSS 無 TSX 改動）：
  - `.avatar-round` 加 `margin: 2px`——黑框是 `box-shadow` 畫在邊框外、不佔版位，用 margin 把那 2px 讓回來，一次修好所有用到圓頭像的地方。
  - 新增 `.tcard:has(.tcard-avatar) .tcard-plate { margin-left: 0.35rem }`——角色卡名牌原本 `margin-left: -6px` 咬進圖窗（全身圖是填滿圖窗的方形，壓上去是設計），但圓頭像四周本來就有留白，咬合會直接壓到黑框上。只在圖窗是圓頭像時取消咬合，全身圖與 emoji 維持原樣。
  - `.opt-target` gap `0.4em` → `0.5em`（composer 目標晶片，即使用者截圖那處）。
  - `.card-editor-avatar` margin-bottom `0.25rem` → `0.6rem`（編輯頁大頭像與下方按鈕列）。
- `:has()` 相容性：App.css 已大量使用 `color-mix()`（門檻 Safari 16.2／Chromium 111），比 `:has()`（Safari 15.4／Chromium 105）更嚴格，故不新增相容風險；即使不支援也只是退回現況，不會壞版。

## Verification
- `npm run build`：`✓ built in 1.05s`（先 `npm install`，容器是新的沒裝過依賴）。
- 視覺驗證：用編譯產物 `dist/assets/index-*.css` 做 before/after 對照頁，Playwright（`/opt/pw-browsers/chromium`）3x 截圖 —— 修改前角色卡名牌確實壓在頭像黑框上、晶片幾乎貼字；修改後三處都有明顯間距，emoji 卡對照組不變。harness 與截圖在本次 session scratchpad，未進 repo。
- 第 1 項間距（原始定位）：`.opt-target` gap `0.4em`（App.css:1111 起）、`.avatar-round` 外框 `box-shadow: 0 0 0 2px var(--avatar-ring)`（App.css:507）吃掉視覺間距；尺寸三處 App.css:513-515。
- 第 2 項改名：全庫 grep `rename` 只有 `rename_world`（data.rs:407、lib.rs:198、指令註冊 lib.rs:946），沒有角色改名路徑；桌名就地編輯 UI 在 App.tsx:2602-2614。
- 第 3 項未儲存：`CharacterCardEditor`（App.tsx:1449 起）無任何 dirty／unsaved 狀態；世界設定的對照實作在 App.tsx:967、1135。`removeImage` App.tsx:1597、`removeAvatar` App.tsx:1607 皆為立即 invoke 寫檔。
- 第 4 項警告：`removeAvatar`（App.tsx:1607）無 `confirm`，直接 `delete_character_avatar`。
- 第 5 項空名：`createCharacter`（App.tsx:2184-2191）只擋 GM／玩家；後端 `validate_name`（data.rs:197-208）對空字串回 `invalid name: ""`。

## Remaining
第 1 項等使用者實機看間距；第 2～5 項未動工。

## Next action
做第 5 項：`createCharacter`（App.tsx:2184）在送 `write_character` 前擋空名（trim 後為空即 return），建卡鈕在空名時 disabled，錯誤字串走 i18n（zh／en 各補一鍵）；順帶檢查 `/`、`\`、開頭 `.`、`..` 等 `validate_name`（data.rs:197）會擋的字元要不要一併前端先提示。之後依序 4 → 3 → 2。

## Constraints
- 沿用 character-image-avatar 既有拍板：頭像存正方形 PNG、圓框走 CSS；移除全身圖不連動刪頭像；刪角色清兩檔。
- 改名須沿用 `validate_name` 規則並擋同名碰撞（比照 data.rs:1897 `renames_world_and_rejects_collisions`）。
