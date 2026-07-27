# Task handoff
Task-ID: scene-history-browser
Updated: 2026-07-24T18:00:00+08:00
Status: done

## Goal
前幕（場景歷史）三件套＋主欄閱讀優先改版（NewPlan §9.4 2026-07-24 拍板：對話訊息上方只留 header 一行，其餘移出主欄；用語統一「幕」）。

## Current state
結案。2026-07-24 使用者驗收通過。含後續零星修訂：chat-main 移除 46rem 上限（主欄填滿左右）、body margin reset（修常駐捲軸）、composer placeholder 精簡、編輯畫面「返回」移至儲存鈕右側。

## Completed
- 第一版（已含在內）：export_scene_markdown＋export_scene command＋8000 字元換幕提醒
- 用語改「幕」：換幕／前幕（{count}）／第 {n} 幕／匯出本幕（i18n 鍵名不變只改字串，en 用 act）
- header：桌名靠左，右側 `.chat-header-actions`（margin-left auto）依序 換幕｜匯出紀錄｜前幕（App.tsx:1327-1364）
- 前幕浮層 `.acts-flyout`（App.tsx:1365-1383）：點前幕展開／再點收起／浮層內「隱藏」鈕，absolute 定位不推擠對話區
- 主欄下半部三選一整面取代（mainView state，App.tsx:772-774、1386-1405）：單幕閱讀 ActReader（標題＋匯出本幕＋返回）／角色卡編輯／世界設定編輯；三種狀態下 messages 與 composer 都不渲染＝不可發言；切桌自動重置（App.tsx:880-882）
- 角色卡編輯：側欄卡改 div role="button"（解 button 巢狀），卡上 ✎ 鈕開整面 EditPane；表單頂顯示卡面圖（characterImages dataURL，無圖顯大顆 emoji，App.tsx:567-575）；show_image checkbox 保留
- GM 卡（暫代世界設定入口）：側欄置頂、🎲、虛線框、不可選為發言對象、僅 ✎ 開 world.md 整面編輯（App.tsx:1203-1212，含「待與世界書合併」註解）
- CSS：acts-flyout／act-reader-header／edit-pane-body／card-editor-avatar*／character-card-gm／character-card-edit／chat-header-actions＋dark mode；舊 scene-history*／scene-viewer-body 刪除

- 二輪修訂（2026-07-24 使用者回饋）：
  - 換幕順手取幕名：summary_messages 指示第一行「標題：…」（transport.rs:142-176）；`extract_scene_title` 純函式解析、失敗整段當摘要不報錯（transport.rs:178-201＋測試）；WorldState 加 `scene_titles`（data.rs:144-154）；begin_next_scene 加 title 參數存舊場景、同次 write_state（data.rs:713-747＋測試）；前端清單與 ActReader 顯示「第 N 幕：幕名」（i18n sceneWithTitle，無幕名退回 sceneLabel）
  - 前幕清單改右側暫時面板：16rem 固定寬、header 下延伸到底、overflow-y auto、隱藏鈕置頂全寬反白區隔、dark mode（App.css:328-378、687-690；App.tsx:1379-1400 chat-body 錨點）
  - 側欄 ✎ 鈕 margin-left auto 靠右（App.css:222-229）
  - textarea 全域 width:100%＋box-sizing＋resize:vertical，只能上下拉、左右貼齊、改字級不變寬（App.css:540-544）

## Verification
- 主線親跑 `cargo test` **47 passed; 0 failed**（新增幕名解析＋存舊場景兩條）、`npm run build` rc=0
- 主線抽查：三選一分支與 composer 僅在對話分支渲染（App.tsx:1386-1472）、切桌重置（App.tsx:881）、GM 卡只有編輯鈕（App.tsx:1203-1212）、編輯畫面卡面圖（App.tsx:567-575）、header 靠右（App.css:432-437）

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main

## Remaining
- 無

## Next action
- 無（任務結案）

## Constraints
- 歷史幕唯讀；整桌匯出與換幕邏輯不變；GM 卡為世界設定暫時入口（待與世界書合併）
