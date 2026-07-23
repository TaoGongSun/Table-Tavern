# Task handoff
Task-ID: scene-history-browser
Updated: 2026-07-24T15:00:00+08:00
Status: in-progress

## Goal
前幕（場景歷史）三件套＋主欄閱讀優先改版（NewPlan §9.4 2026-07-24 拍板：對話訊息上方只留 header 一行，其餘移出主欄；用語統一「幕」）。

## Current state
閱讀優先改版完成（Opus subagent 實作、主線驗收）：npm build 綠、cargo test 45 綠（後端零改動）。剩使用者視覺驗收。

## Completed
- 第一版（已含在內）：export_scene_markdown＋export_scene command＋8000 字元換幕提醒
- 用語改「幕」：換幕／前幕（{count}）／第 {n} 幕／匯出本幕（i18n 鍵名不變只改字串，en 用 act）
- header：桌名靠左，右側 `.chat-header-actions`（margin-left auto）依序 換幕｜匯出紀錄｜前幕（App.tsx:1327-1364）
- 前幕浮層 `.acts-flyout`（App.tsx:1365-1383）：點前幕展開／再點收起／浮層內「隱藏」鈕，absolute 定位不推擠對話區
- 主欄下半部三選一整面取代（mainView state，App.tsx:772-774、1386-1405）：單幕閱讀 ActReader（標題＋匯出本幕＋返回）／角色卡編輯／世界設定編輯；三種狀態下 messages 與 composer 都不渲染＝不可發言；切桌自動重置（App.tsx:880-882）
- 角色卡編輯：側欄卡改 div role="button"（解 button 巢狀），卡上 ✎ 鈕開整面 EditPane；表單頂顯示卡面圖（characterImages dataURL，無圖顯大顆 emoji，App.tsx:567-575）；show_image checkbox 保留
- GM 卡（暫代世界設定入口）：側欄置頂、🎲、虛線框、不可選為發言對象、僅 ✎ 開 world.md 整面編輯（App.tsx:1203-1212，含「待與世界書合併」註解）
- CSS：acts-flyout／act-reader-header／edit-pane-body／card-editor-avatar*／character-card-gm／character-card-edit／chat-header-actions＋dark mode；舊 scene-history*／scene-viewer-body 刪除

## Verification
- 主線親跑 `cargo test` 45 綠、`npm run build` rc=0
- 主線抽查：三選一分支與 composer 僅在對話分支渲染（App.tsx:1386-1472）、切桌重置（App.tsx:881）、GM 卡只有編輯鈕（App.tsx:1203-1212）、編輯畫面卡面圖（App.tsx:567-575）、header 靠右（App.css:432-437）

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main

## Remaining
- 使用者視覺驗收：header 右排三鈕；前幕浮層兩種收法；點幕整面閱讀（無輸入列）＋返回；角色卡 ✎ 整面編輯含卡面圖；GM 卡編輯世界設定；以上狀態切桌自動復原

## Next action
- 使用者驗收通過即結案

## Constraints
- 歷史幕唯讀；整桌匯出與換幕邏輯不變；GM 卡為世界設定暫時入口（待與世界書合併）
