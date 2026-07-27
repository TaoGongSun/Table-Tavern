# Task handoff
Task-ID: post-mvp-st-import
Updated: 2026-07-23T22:10:00+08:00
Status: done

## Goal
依 NewPlan §5.2：匯入 SillyTavern V2 角色卡（內嵌 JSON 的 PNG 或純 JSON）。欄位對應到本專案卡格式；對不上的欄位保留原始資料；匯入 PNG 原圖存世界目錄並做角色圖顯示／隱藏開關（UI 在左側欄角色卡，NewPlan §9.4）。只做匯入不做匯出。

## Current state
結案。2026-07-23 使用者實測通過：匯入範例卡「夜鶯」→ 側欄角色卡顯示 PNG 圖 → CardEditor 關「顯示角色圖片」→ 換回 emoji 頭像（附截圖驗證）。

## Completed
- 後端解析＋欄位對應＋`import_character` command（上一手完成，見 git 7c4f93f）
- `show_image` 旗標進卡 frontmatter（本次，主線直做）：
  - CharacterMeta／CharacterCard 加 `show_image: bool`，serde default true（data.rs:100-125）；parse_frontmatter 認 `show_image` 鍵、缺鍵視為 true（data.rs:371、386）；serialize 固定寫出（data.rs:449）
  - 匯入卡與手建卡預設 true；舊卡檔不需遷移
- 後端角色圖讀取：`character_image(root, world, name) -> Option<base64>` 讀 `characters/<name>.png`（import.rs:59-66）；base64_encode 從測試模組移成正式碼共用（import.rs:196-217）；lib.rs 掛 `read_character_image` command 並註冊
- 前端（App.tsx）：
  - 「匯入卡」鈕＋隱藏 file input（accept .png/.json）放側欄建卡表單旁；讀 bytes → `invoke("import_character")`，顏色沿用 PALETTE 輪選；成功後刷新角色列表＋選中新角色
  - 角色圖快取 `characterImages`（name → data URL），進桌與匯入後載入；側欄角色卡 `show_image && 有圖` 時顯示 `<img>`，否則 emoji Avatar
  - CardEditor 加「顯示角色圖片」checkbox（僅有圖的角色顯示），存卡即寫回 frontmatter
- i18n 三鍵（showImageLabel／importCard／importCardHint）zh-TW＋en；App.css 加 `.character-card-image`（object-fit: cover、對齊頂部）
- 驗收用範例卡：scratchpad 已產一張含真實圖面＋內嵌 V2 JSON 的 96×96 PNG（角色「夜鶯」，含 character_book 條目），已傳給使用者

## Verification
- `cd src-tauri && cargo test`：**40 passed; 0 failed**（新增 import::tests::character_image_returns_png_base64_or_none、data::tests::show_image_false_round_trips_and_missing_key_defaults_to_true；兩條舊測的 frontmatter 斷言已補 show_image 行）
- `npm run build`：rc=0，tsc＋vite 皆綠
- 範例卡 PNG 經 sips 驗證為有效影像（96×96）

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main

## Remaining
- 無

## Next action
- 無（任務結案）

## Constraints
- 不做匯出相容（NewPlan §5.2）；不加新 crate 依賴（base64 編解碼皆手寫已含測試）
- 對不上的欄位以原始檔整份保留（`<name>.png`／`<name>.import.json`），不塞進卡內文
