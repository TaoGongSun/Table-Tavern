# Task handoff
Task-ID: transcript-export
Updated: 2026-07-23T22:10:00+08:00
Status: in-progress

## Goal
一鍵匯出跑團紀錄：整桌全部場景 JSONL 依序轉可讀 Markdown，儲存位置由使用者以原生「另存新檔」對話框自選，存檔後在檔案總管顯示。只讀不改正典資料；格式只做 Markdown（YAGNI）。

## Current state
前後端完成、測試建置全綠。2026-07-23 依使用者回饋把「固定寫下載資料夾」改成「另存新檔對話框自選位置」（順帶消除同名覆蓋疑慮）。剩使用者實測一次存檔流程即結案。

## Completed
- data.rs：`export_transcript_markdown(root, world, lang)`（data.rs:536 起）——場景號數值排序、zh-TW／en 雙語標題、dialogue/player `**speaker**：text`、narration blockquote、system 斜體括號、無紀錄回 Err；測試在 data.rs:961 起（雙語各一組＋無紀錄桌）
- lib.rs：`export_transcript(world, path)` command——讀 config 取語系、產 Markdown 寫入前端傳來的路徑；檔名與位置改由前端 save 對話框決定
- App.tsx `exportTranscript()`：`save()`（@tauri-apps/plugin-dialog）跳另存對話框，預設檔名「桌名 跑團紀錄 YYYY-MM-DD HHMM.md」（en 語系為 transcript，i18n key exportFileName）、filter 限 .md；取消即中止；成功後 `revealItemInDir(path)`
- 新增依賴 tauri-plugin-dialog 2（Cargo.toml＋lib.rs init＋capabilities/default.json `dialog:default`＋npm @tauri-apps/plugin-dialog）——原生存檔對話框無法不靠 plugin 實作，屬必要新增

## Verification
- `cargo test`：**40 passed; 0 failed**、`cargo check --all-targets` 0 warning
- `npm run build`：rc=0（tsc＋vite 綠）
- 接線：lib.rs `export_transcript(world, path)`＋App.tsx `save()`→`invoke`→`revealItemInDir`

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main

## Remaining
- 使用者實測：按「匯出紀錄」→ 跳另存對話框（預設檔名含桌名＋日期時分）→ 選位置存檔 → 檔案總管跳出該檔；按取消則無事發生

## Next action
- 使用者實測通過即結案

## Constraints
- 只讀 transcript；不做匯入回讀；新依賴僅 tauri-plugin-dialog（原生對話框必要）
