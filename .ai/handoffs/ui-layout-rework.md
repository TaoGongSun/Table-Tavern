# Task handoff
Task-ID: ui-layout-rework
Updated: 2026-07-23T22:40:00+08:00
Status: done

## Goal
依 NewPlan §9.4（2026-07-23 拍板）重構主版面：左側欄以角色卡為主體（直排頭像＋名稱、預留角色圖片位、點選即選定發言）、移除聊天區上方水平 cast-row、桌列表收進側欄可摺疊區塊（摺疊狀態存 localStorage）。純前端，不動後端。

## Current state
結案。2026-07-23 使用者視覺驗收通過（截圖示角色卡左欄直排＋桌列表摺疊／展開皆正常）。實作由 Codex（gpt-5.6-terra）依主線規格完成，主線複驗。

## Completed
- 側欄重排（src/App.tsx:785-855）：`<details class="table-section">` 摺疊桌區（summary＋開新桌鈕＋桌列表）→ `<section class="character-panel">` 直排角色卡＋建卡表單 → 原 sidebar-footer（語言下拉＋Settings）不動
- 摺疊狀態 localStorage：key `table_list_open`（App.tsx:61 定義、523 初始化、791 寫回），仿 `sidebar_width` 模式，未進 config
- 角色卡：頭像獨立 `.character-card-avatar` 固定寬 4rem 一格（App.css:236-244），日後換角色圖片不必改結構；選中沿用 `--ring` 邊框＋粗體（App.css:230-234）
- 移除 cast-row：App.tsx 舊 section 整段刪除，App.css 的 `.cast-row`／`.cast`／`.cast-active` 一併刪除，無死 CSS
- 側欄捲動改內部分區：`.sidebar` overflow hidden，桌列表上限 30vh 內捲、角色卡列表佔剩餘高度內捲（App.css:95、142-144、211-217）
- 未新增 i18n key（摺疊標題重用 `tableListAria`、角色區 aria 重用 `castAria`）

## Verification
- `npm run build`（tsc＋vite）rc=0，`✓ built in 385ms`（主線親跑）
- `grep -c "cast-row" src/App.tsx src/App.css` 兩檔皆 0
- 既有功能逐項行號核對：點角色選發言 App.tsx:819、建卡 submit App.tsx:829、切桌 App.tsx:804、開新桌 App.tsx:796、桌改名 App.tsx:889、側欄拖曳 App.tsx:858（邏輯未動）、語言下拉 App.tsx:840、Settings App.tsx:853
- Codex 產生的 scaffolding 複本（AGENTS.md、.agents/）已刪除，git status 乾淨只剩本任務變更

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main

## Remaining
- 使用者以 `npm run tauri dev`（或重新打包）視覺驗收：摺疊開合並重啟後狀態保留、角色卡選中視覺、側欄窄寬兩檔下卡片不破版

## Next action
- 使用者視覺驗收通過即結案；若要調間距／卡片高度屬純 CSS 微調（`.character-card` 系列在 App.css:219-250 附近）

## Constraints
- 純視覺狀態（摺疊、側欄寬度）只進 localStorage，不進 config
- 不得移除既有功能（點名發言、建卡、桌改名、側欄拖曳調寬）
- 此任務定下設定鈕與角色圖片落點：ui-settings-panel 與 post-mvp-st-import 的圖片顯示接續在此版面上做
