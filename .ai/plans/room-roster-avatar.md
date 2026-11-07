# room-roster-avatar：名冊頭像退回角色圖（方案）

來源：手機玩家回饋「網頁端沒有顯示頭像」。查證：手機上傳與裁切那條路完整（上傳 → 執行端寫角色圖與 `<id>.avatar.png` → 重送名冊 → 座位列與入座名單用圖），但 `room/seed.rs` 的名冊頭像**只讀 `<id>.avatar.png`**，這個檔只有手機上傳過圖、或房主在桌面版按過「製作頭像」才存在；桌面版匯入的酒館卡只寫 `<id>.png`（`import/card.rs::store_card_source`），一句話生成與手機「建立角色」沒有圖。結果房主電腦上匯入的卡都有大圖，朋友手機上入座名單一片空白、座位列只有字母圈。作者裁決：做〔作者裁決 2026-11-07〕。

案別：小案，核心估約 40 行（執行端 Rust 約 25、手機 TS／CSS 約 15）。不做方案審，做完 Sol 驗收一次、Sonnet 用假伺服器拍手機截圖主線親看。

## 1 執行端（`src-tauri/src/room/seed.rs`）

- `avatar()` 找不到 `<id>.avatar.png` 時改讀角色圖 `<id>.png`，同一條 96px JPEG 縮圖、同一個 16 KiB 上限；`avatar.png` 有就優先（房主裁過的圓形頭像比整張圖好）。
- 退路只在卡的 `show_image` 為真時用：房主在編輯器關掉「顯示圖」就是不想露那張圖〔模型判斷·未裁決〕。`roster()` 已讀到卡，把旗標傳進去。
- 2:3 角色圖 `thumbnail(96, 96)` 等比縮成 64×96；手機 CSS 已是 `object-fit: cover; object-position: top`，圓框保住上半部的臉。

## 2 手機入座名單（`party/src/features/join/SeatPicker.tsx`）

- 沒頭像時畫字母圈（角色名首字、邊框用卡色），跟座位列同一個 `Avatar`：把 `SeatStrip.tsx` 的 `Avatar` 匯出共用，入座名單用 44px 版本（既有 `.seat-card img` 的尺寸與對齊規則搬到 `.seat-card .seat-avatar`）。
- 故事列不加頭像〔模型判斷·未裁決〕：手機劇情區寬度有限，名字加卡色邊框已能認人。

## 3 驗證

- `src-tauri/src/room/tests.rs`（或 `seed` 旁）：有 `<id>.png` 沒 `avatar.png` → 名冊 `avatar` 是 `data:image/jpeg` data URL；兩者都有 → 用 `avatar.png` 那張（兩張不同尺寸可分）；`show_image=false` 且只有 `<id>.png` → `null`；都沒有 → `null`。
- `party/src/features/room/room.test.tsx` 或 join 測試：名冊卡 `avatar: null` 時入座名單那列有字母圈（首字）；有 `avatar` 時是 `<img>`。
- `cd src-tauri && cargo test --lib room && cargo fmt`；`cd party && npm run typecheck && npm test && npm run format`；結案前主線 `npm run verify`。

## 4 觀察（範圍外，不立案）

- 桌面版自己的對話框頭像（`characters.avatars[speaker]`）同樣只讀 `avatar.png`，匯入的卡沒按「製作頭像」就是 emoji；單人版行為，不在本案。

## 5 施工結果

- 核心約 16 行（Rust 11：`seed::avatar` 多收 `show_image`、`avatar.png` 優先否則讀 `<id>.png`；手機 5：`Avatar` 匯出共用、`.seat-card .seat-avatar` 44px）、測試 +93（Rust 四種情況、手機兩種）。Grok 施工 1 輪、Sol 驗收 1 輪 PASS（無必改）、Sonnet 假伺服器截圖 2 張主線親看：入座名單三個 44px 圓對齊、字母圈帶卡色、座位列 22px 不受影響。
- 執行端退路沒有端對端實測（假伺服器拍不到執行端），靠 Rust 測試：2:3 角色圖縮成 64×96 JPEG、裁過的圓圖優先、關「顯示圖」不送角色圖。
