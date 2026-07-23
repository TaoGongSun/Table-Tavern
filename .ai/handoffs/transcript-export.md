# Task handoff
Task-ID: transcript-export
Updated: 2026-07-23T16:20:00+08:00
Status: in-progress

## Goal
一鍵匯出跑團紀錄：整桌全部場景 JSONL 依序轉可讀 Markdown，存到使用者下載資料夾並在檔案總管顯示。只讀不改正典資料；格式只做 Markdown（YAGNI）。

## Current state
前後端程式碼完成、測試與建置全綠。剩使用者在真實 App 按一次匯出鈕實測（下載資料夾出檔＋檔案總管跳出）。實作由 Codex（gpt-5.6-terra）依主線規格完成。

## Completed
- data.rs：`export_transcript_markdown(root, world, lang)`（data.rs:536 起）——場景號數值排序、zh-TW／en 雙語標題、dialogue/player `**speaker**：text`、narration blockquote、system 斜體括號、無紀錄回 Err；測試在 data.rs:961 起（雙語各一組＋無紀錄桌）
- lib.rs：`export_transcript` command（lib.rs:114）——讀 config 取語系、寫檔到 download_dir（檔名含日期時分不撞名）、回傳完整路徑；generate_handler 註冊（lib.rs:329）
- App.tsx：chat-header 匯出鈕（App.tsx:906 附近），onClick 呼叫 command 後 `revealItemInDir(path)`（App.tsx:642-643）；失敗走既有 setError
- capabilities/default.json 加 opener reveal 權限（default.json:6）；i18n.ts 新增匯出鈕 key（zh-TW＋en，i18n.ts:82 附近）

## Verification
- 主線親跑 `cargo test`：**38 passed; 0 failed**（含新匯出測試；Codex 沙盒擋 TCP 的 transport 測試在正常環境為綠）
- 主線親跑 `npm run build`：`✓ built in 398ms`
- grep 證實接線：export_transcript command（lib.rs:114、329）、前端呼叫＋reveal（App.tsx:642-643）

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main

## Remaining
- 使用者實測：開一桌有紀錄的桌 → 按匯出鈕 → 下載資料夾出現 `<桌名> 跑團紀錄 <日期時分>.md` 且檔案總管跳出；en 語系標題為英文

## Next action
- 使用者實測通過即結案

## Constraints
- 只讀 transcript，衍生檔只落 download_dir；不做匯入回讀；無新依賴（opener 為既有套件）
