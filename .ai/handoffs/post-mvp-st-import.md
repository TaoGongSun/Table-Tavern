# Task handoff
Task-ID: post-mvp-st-import
Updated: 2026-07-23T15:45:00+08:00
Status: in-progress

## Goal
依 NewPlan §5.2：匯入 SillyTavern V2 角色卡（內嵌 JSON 的 PNG 或純 JSON）。欄位對應到本專案卡格式；對不上的欄位保留原始資料；匯入 PNG 原圖存世界目錄並做角色圖顯示／隱藏開關（UI 在左側欄角色卡，NewPlan §9.4）。只做匯入不做匯出。

## Current state
後端切片完成：解析＋欄位對應＋`import_character` command＋測試全綠。前端（匯入鈕、檔案讀取、角色圖顯示／隱藏開關）尚未動工，等 ui-layout-rework 視覺驗收後接。實作由 Codex（gpt-5.6-terra）依主線規格完成，主線已親自複驗。

## Completed
- 新模組 src-tauri/src/import.rs：`import_character(root, world, bytes, color)`（import.rs:8）
  - PNG magic 判別 → tEXt `chara` chunk 走訪（含長度溢位／越界防護，import.rs:101-127）→ 手寫 base64 解碼（import.rs:129-174，無新依賴）；非 PNG 直接當 JSON
  - V2（`data` 包裝）與 V1（頂層欄位）都吃（import.rs:21-24）
  - 欄位對應（主線拍板）：description/personality/scenario/first_mes/mes_example → `## 公開` 內 `### 簡介／人格與語氣／場景／開場白／語氣範例` 各節（import.rs:59-74）；character_book entries → `## 私有` 逐條 `- **keys**：content`（import.rs:76-99）；color 參數傳入、avatar 🎭、tier default（同前端手建卡）
  - 同名已存在 → 報錯不覆蓋不改名（import.rs:31-34）
  - 原始檔保留：PNG 原 bytes 存 `characters/<name>.png`、純 JSON 存 `characters/<name>.import.json`（import.rs:45）；`list_characters` 只讀 .md 不受干擾（data.rs:398-399 既有行為）
- lib.rs 掛接：`mod import`（lib.rs:3）、`#[tauri::command] import_character`（lib.rs:82-91）、generate_handler 註冊（lib.rs:305）
- data.rs 僅把 invalid_data／validate_name／character_path 改 `pub(crate)`（data.rs:97、106、123），無行為變更

## Verification
- 主線親跑 `cd src-tauri && cargo test`：**36 passed; 0 failed**（含 import::tests 5 條：V2 JSON＋原檔落地、PNG chunk＋原檔落地＋list_characters 不受干擾、V1 fallback、同名不覆蓋、base64 向量＋非法輸入；import.rs:242-338）
- Codex 沙盒跑出的唯一紅測（transport mock TCP listener 被 Operation not permitted 擋）在正常環境為綠，非本次改動引入

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main

## Remaining
- 前端：匯入鈕（放側欄角色卡區，建卡表單旁）＋讀檔（HTML file input 讀 bytes 傳 `invoke("import_character", { world, data, color })`，color 沿用 PALETTE 輪選邏輯 App.tsx:649）
- 角色圖顯示／隱藏開關：卡 frontmatter 或另存旗標待定；左側欄 `.character-card-avatar`（App.css）已預留圖片格
- 以上兩項等 ui-layout-rework 視覺驗收通過後動工

## Next action
- ui-layout-rework 結案後：前端接 `import_character`（command 介面已就緒：world: String, data: Vec<u8>, color: String → CharacterMeta）

## Constraints
- 不做匯出相容（NewPlan §5.2）；不加新 crate 依賴（base64／PNG 解析皆手寫已含測試）
- 對不上的欄位以原始檔整份保留（`<name>.png`／`<name>.import.json`），不塞進卡內文
