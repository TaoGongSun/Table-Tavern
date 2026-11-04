# room-player-cards — 多人房間階段二：玩家匯入／編輯自己的卡

案別：大案，核心估約 850 行（三包），方案與驗收都送審。方案與協定細節見 [plans/room-player-cards.md](../plans/room-player-cards.md)（§3.6 是包 1 定下的協定，包 2、3 照它接）。階段一見 [交接檔](archive/room-host-desktop-play.md)。

## 現況
- 包 1（伺服器）施工完：`party/server/core/cards.ts`（上傳兩段式、取檔、讀卡／存卡／生圖轉送、名額回收）、`core/upload.ts`（兩殼共用的收件），Workers 與 Node 兩殼接上 `/cards` 路由。party test、typecheck、verify 綠；等 Sol 驗收。
- 手機錯誤碼新增的 7 個暫時對到通用錯誤（`party/src/features/room/error-text.ts`），包 3 換成各自說明。

## 已驗
- 本機 `wrangler dev`：DO 讀得到 `Content-Length`；上傳 202、取檔一次後 404；8 MiB 上傳與取檔正常；9 MiB 回 413、沒長度回 411、401／403／415 都收得到。

## 下一步
- 包 1 驗收修正輪；VN 模式進 main 後 rebase 一次。
- 包 2（執行端，Rust `room/cards.rs`）、包 3（手機編卡畫面＋桌面資訊列）。
