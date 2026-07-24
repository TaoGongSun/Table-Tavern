# Task handoff
Task-ID: post-mvp-character-archive
Updated: 2026-07-24T18:40:00+08:00
Status: in-progress

## Goal
角色卡隱藏區（軟刪除）＋還原＋僅限隱藏區的真刪除（確認框明示不可復原）；GM 上下文與點名一律只見在場角色。

## Current state
功能全部完成、主線本機驗證全綠，只剩使用者實測 UI 驗收。儲存形式拍板：角色卡 frontmatter 加 `archived` 布林旗標（不搬目錄，PNG 原地不動）。

## Completed
- data.rs：CharacterMeta／CharacterCard 加 `archived`（serde default false，舊卡缺欄位照常 parse）；parse／serialize 支援；新增 `set_character_archived`、`delete_character`（.md＋同名 .png 一併刪，走 character_path 驗名）。
- lib.rs：assemble_gm 過濾 archived（GM roster 與 suggest 自然排除）；新增並註冊兩個同名 tauri commands。
- App.tsx：側欄在場列表改用 activeCharacters；編輯畫面「收起角色」鈕；隱藏區 details（有隱藏角色才顯示）含還原／刪除；刪除走 plugin-dialog confirm；speaker 被收起時自動改指第一個在場角色；gmAdvance／送話按鈕判斷改用在場數。
- i18n.ts：zh-TW＋en 六個新 key（含刪除確認文案，明示不可復原）；App.css 隱藏區樣式。

## Verification
- 主線本機 `cd src-tauri && cargo test`：48 passed; 0 failed（含新測試：archived round-trip、舊卡預設 false、delete 後 .md/.png 皆不存在）。
- `npm run build`：✓ built in 400ms，無 TS 錯誤。
- GM 過濾證據：src-tauri/src/lib.rs:286 `.filter(|meta| !meta.archived)`。
- transcript 讀寫函式零改動（git diff 確認）。

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main

## Remaining
使用者實測：收起→在場列表消失且 GM 不再點名；隱藏區還原；隱藏區刪除跳確認框、確認後角色與圖檔消失。

## Next action
請使用者 `npm run tauri dev` 實測上述三條路徑，通過即結案。

## Constraints
不改 transcript；UI 字串全走 i18n；真刪除必經 plugin-dialog confirm 且文案明示不可復原。
