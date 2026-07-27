# Handoff: drag-reorder-lists

## Current state
**已結案**（2026-07-27 使用者九項實測全通過）。三件全數完成：角色卡拖曳排序（GM 固定最上）、世界書條目拖曳並刪除 ↑↓ 按鈕、兩處列禁反白。

## Completed
- **共用拖曳判定** `useDragReorder`（src/App.tsx:985-1065）：pointer 事件、移動門檻 5px（`DRAG_THRESHOLD_PX`，src/App.tsx:985）、一次只與相鄰列交換（越過鄰居中線才換，換完中線落到指標另一側，故高度不一也不抖）、`justDragged()`（src/App.tsx:1057）讓拖曳結束那一下的 click 不觸發選發言者（旗標於 `setTimeout(…, 0)` 解除，因為 click 在 pointerup 之後才補送）。按在 `button/a/input/textarea/select` 上不啟動拖曳。
- **角色卡排序後端**：`CharacterMeta.display_index`（`#[serde(skip)]`，只在後端流通，data.rs:117）＋ frontmatter 欄位 `display_index`（解析 data.rs:949、寫出 data.rs:1025）；`list_characters` 改依 `display_index` 排、無索引的舊卡退回名字排；新指令 `reorder_characters`（data.rs:1084、lib.rs:272、註冊 lib.rs:980）；`write_character` 先 `ensure_display_indices`（data.rs:1033）整批補齊舊卡，再由 `display_index_for`（data.rs:1043）取自己的索引——既有卡保留原位、新卡排最後（data.rs:1138-1139）；`rename_character` 不動排序位置（data.rs:1195）。
- **世界書排序後端**：`move_worldbook_entry`（相鄰交換）整支換成 `reorder_worldbook_entries`（一次重寫 displayIndex，支援跨多格；沒送到的 uid 依原順序接在後面）——data.rs:792、lib.rs:236、註冊 lib.rs:975。
- **前端接線**：角色卡 `castDrag`（src/App.tsx:2213）＋`reorderCast`（樂觀套用、失敗回捲，src/App.tsx:2544）＋渲染 src/App.tsx:2809；世界書 `entryDrag`（src/App.tsx:1080）＋`reorderEntries`（同樣樂觀＋回捲，src/App.tsx:1225）＋渲染 src/App.tsx:1436；世界書 ↑↓ 兩顆按鈕刪除。
- **CSS**：`.tcard` 加 `user-select: none`（App.css:506-508）、`.worldbook-row` 加 `cursor: grab`＋`user-select: none`（App.css:1422-1425）、拖曳中共用 `.row-dragging`（浮起：scale 1.02＋加深陰影＋grabbing 游標，App.css:512）。**兩處都要寫 `-webkit-user-select`**（commit 5e5e803）：正式 build 由 esbuild 自動補前綴（改前後 CSS 產物位元組相同、雜湊檔名沒變即是證據），但 `npm run tauri dev` 走 vite dev server 原樣送 CSS 不補前綴，macOS 的 WKWebView 就忽略無前綴版而仍可反白。
- **i18n**：刪 `worldbookMoveUp`／`worldbookMoveDown`，新增 `dragToReorder`（zh「按住拖曳可調整順序」src/i18n.ts:133／en「Drag to reorder」src/i18n.ts:382），掛在兩處列的 title。

## Verification
- `cargo test`：96 passed; 0 failed（改版前 93）。新增／改寫測試：`reordering_worldbook_entries_applies_the_given_order`（跨多格來回）、`reordering_worldbook_keeps_unlisted_entries_after_the_listed_ones`（不存在的 uid 忽略、沒送到的接後面）、`reordering_legacy_worldbook_entries_normalizes_display_indices`、`reordering_worldbook_entries_preserves_order_and_unknown_fields`（SillyTavern 的 `order`／未知欄位不被吃掉）、`reordering_characters_persists_order_and_survives_rename`（建卡順序＝初始順序、沒送到的接後面、改名不移位、重存不移位、新卡排最後）、`saving_one_legacy_card_does_not_reshuffle_the_others`（舊卡回歸防護）。
- `npm run build`：tsc 綠＋`✓ built in 472ms`。
- `cargo clippy --all-targets`：5 個 warning 全是既有的（import.rs:203、lib.rs:474、data.rs:47 等），本次新增程式碼零 warning。
- **使用者 dev 實測九項全通過**（2026-07-27）：跨多格拖曳、點擊仍選發言者且拖完不誤選、GM 卡拖不動也擋得住、世界書拖曳與 ↑↓ 消失、拖曳視覺、世界書順序退出再進保留、角色卡順序關 app 重開保留、回歸三項（編輯存檔不跳位／改名不移位／新卡在最下面）、移除圖片確認框。
- 實測退回一項並已修：拖曳中的 `transform: scale(1.02)` 讓列寬溢出被外層 `overflow` 裁掉左右邊框，改成只留陰影＋握拳游標（commit e4e3172）。

## 未做（超出本任務範圍，要的話另立）
- 拖到清單邊緣時自動捲動。
- 觸控裝置支援：沒設 `touch-action: none`，以免側欄在觸控上不能捲。

## Constraints
同 tasks/drag-reorder-lists.md。
