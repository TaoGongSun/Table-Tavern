# Task
Task-ID: party-prettier
Title: party/ 導入 prettier（100 寬）與 verify 檢查，整批 format-only 重排
Status: done
Created: 2026-11-04
案別：小案（只改設定＋純格式重排）

## 要做的事
party/ 手寫風格是 100 寬但沒有格式化工具，每次改檔格式各自為政。導入 prettier（printWidth 100，貼近現有寫法）、`npm run verify` 加格式檢查只管 party/，整批重排放一筆 format-only commit；順手改 `party/server/workers/wrangler.toml` 過時註解。根目錄 app 不在本案範圍。

## 現在狀態
- 已結案進 main（設定一筆＋純格式重排一筆）。party/ 用 prettier 3.6.2（`.prettierrc.json`：100 寬、雙引號、分號、尾逗號 all），`npm run verify` 第一個 party 步驟跑 `format:check`；改 party 的檔後跑 `cd party && npm run format`。根目錄 app 沒有導入。
