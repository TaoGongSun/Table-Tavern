# Task handoff
Task-ID: scene-history-browser
Updated: 2026-07-24T13:10:00+08:00
Status: in-progress

## Goal
長團體驗三件套（2026-07-24 使用者提議拍板）：過去的場收合列表、單場唯讀檢視＋單場匯出、紀錄過長建議換場提醒。整桌匯出保留不動。

## Current state
程式碼完成（Opus subagent 實作、主線驗收）：cargo test 45 綠、npm build 綠。剩使用者視覺驗收。

## Completed
- data.rs：export_transcript_markdown 抽共用 helper（render_transcript_entry／render_scene_section，行為不變）＋新增 `export_scene_markdown(root, world, scene, lang)`（單場輸出、場景不存在回 Err）；測試兩條（只含該場事件、缺場報錯）
- lib.rs：`export_scene(world, scene, path)` command（lib.rs:133 起），generate_handler 已註冊；讀單場沿用既有 read_transcript command
- App.tsx：`SceneViewer` modal（App.tsx:618 起，唯讀事件列表＋「匯出本場」走 save 對話框＋revealItemInDir）；chat-header 下 `.scene-history` 收合列（App.tsx:1302-1313，scene>0 才出現，場號顯示從 1 起算）；換場提醒 `SCENE_LENGTH_HINT_CHARS = 8000` 字元門檻（App.tsx:82、1122-1124、1291）
- App.css：scene-history／scene-history-list／scene-length-hint／scene-viewer-body／scene-event 樣式；i18n 五鍵（pastScenes／sceneLabel／exportScene／sceneExportFileName／sceneTooLongHint）zh＋en

## Verification
- 主線親跑 `cargo test`：**45 passed; 0 failed**
- 主線親跑 `npm run build`：rc=0
- 主線抽查：export_scene 註冊與寫檔流程（lib.rs:124-136）、列表 scene>0 條件與場號 +1 顯示（App.tsx:1302-1308）、匯出本場 save→invoke→reveal（App.tsx:646-658）、提醒門檻常數與渲染（App.tsx:82、1291）

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main

## Remaining
- 使用者驗收：換過場的桌 header 下應見「過去的場（N）」收合列 → 點「第 1 場」開唯讀視窗 → 「匯出本場」出檔名帶場次的另存對話框；當前場文字量超過 8000 字元時換場鈕旁出現小字提醒

## Next action
- 使用者驗收通過即結案

## Constraints
- 歷史場景唯讀；整桌匯出行為不變；提醒不擋操作
