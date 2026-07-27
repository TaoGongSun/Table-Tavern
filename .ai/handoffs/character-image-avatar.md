# Handoff: character-image-avatar

## Current state
實作完成（commit fa4d855），cargo test 80 綠＋npm build 綠＋tauri dev 冒煙通過，等使用者實測後結案。

## Completed
- 2026-07-27 規格拍板＋`react-easy-crop@6.2.3` 入依賴。
- 後端：import.rs 五個 fn（save/delete 全身圖、read/save/delete 頭像，共用 save_character_png／delete_character_png helper）＋lib.rs 五個 Tauri 指令註冊；data.rs delete_character 連 `<name>.avatar.png` 一起清；新增測試 4 條（往返、拒非 PNG／不存在角色、刪除冪等、刪角色清頭像）。
- 前端：App.tsx CropDialog（modal 結構沿用、zoom 滑桿、canvas 輸出 PNG）；CardEditor 動作列（加入／更換／移除圖片、製作／移除頭像）；characterAvatars 快取併入 loadCharacterImages；tcard 圖窗／發言者列／編輯頁三處 emoji 位改吃圓頭像；i18n zh/en 各 10 鍵；CSS `--avatar-ring: #000`＋`.avatar-round`。
- 主線修正：移除 codex 烘進 PNG 的圓形 clip——頭像存正方形原樣，圓形黑框純 CSS（App.tsx CropDialog confirmCrop 內註解）。

## Verification
- `cargo test`（本機）：`test result: ok. 80 passed; 0 failed`（codex 沙箱那條 transport mock 失敗係沙箱禁 TCP，本機通過）。
- `npm run build`：`✓ built in 428ms`。
- `npm run tauri dev`：app 進程 10 秒內起、無 panic／error（冒煙）。
- 指令 wiring 主線逐一對過：前端 invoke 五名稱＝lib.rs invoke_handler 註冊（lib.rs:321 起、:700 附近清單）。

## Remaining
- 使用者實測：加入圖片→裁切→顯示；製作頭像→列表／發言者列顯示圓頭像黑框；移除頭像回 emoji；移除／更換圖片；已匯入角色重新裁切。通過即結案。

## Next action
等使用者實測回報；有問題依回報修，無問題任務結案（TASKS.md 移 Done）。

## Constraints
- 頭像存 256×256 正方形 PNG（`<name>.avatar.png`），圓形與黑框由 CSS `--avatar-ring`（#000）畫，不烘進圖檔。
- 全身圖鎖 2:3、寬上限 1024，覆寫 `characters/<name>.png`（匯入卡 metadata 已抽進 md，覆寫可接受）。
- 頭像只能從該角色全身圖裁，不另選圖（使用者拍板，對齊 AI 生圖規劃）；移除全身圖不連動刪頭像；刪角色兩檔都清。
