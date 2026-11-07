# Task
Task-ID: multiplayer-friends-test
Title: 給朋友測試的多人連線版：房間預設正式伺服器、簡中下載頁以多人連線為主軸
Status: 已上線，等朋友實機測試
Created: 2026-11-03
案別：小案（核心估 ~100 行）

## 目標
- 桌面版房間面板預設正式伺服器 `https://tt-room.pirahus.com`（仍可改，自架用）。
- 簡中下載頁以多人連線介紹（房主桌面版開房出額度當 GM、朋友手機點邀請連結加入），附簡中房主／連線者截圖；其他九語維持原樣〔作者只要求簡中〕。
- 簡中下載頁只留 pCloud，不出 GitHub 按鈕〔作者裁決 2026-11-03〕；版本行用建置時的桌面版版本號、不打 GitHub〔作者裁決 2026-11-04〕。
- 測試通道端到端：房主連正式伺服器開房、手機尺寸加入選角送行動、看到 GM 串流（同時拍截圖）。

## 現況
- 房間伺服器已部署：Workers 自訂網域 `tt-room.pirahus.com`（`wrangler.toml` routes，workers.dev 關閉）；部署指令 `CLOUDFLARE_ACCOUNT_ID=<帳號 id> npx wrangler deploy --config server/workers/wrangler.toml`（在 `party/` 先 `npm run build`）。暫用網域，換專用網域見 [room-server-own-domain](../tasks/room-server-own-domain.md)。
- 房間預設網址在 Rust `room::status()`（沒開過房時回官方網址；面板與 `room_enter` 都讀它）。
- 執行端沒有固定模型名（智慧免費、CLI 預設）時送 `default`，手機與桌面面板顯示在地化（简中「默认」、繁中「預設」）。
- 簡中頁：主標／導言改多人連線（zh-CN 字典）、特色卡前加「怎么和朋友一起玩」兩張卡（文案固定簡中，`tour-features.ts` 的 `ROOM_TOUR_ZH_CN`）、只給 pCloud；簡中只用 `*.zh-CN.webp`，沒拍的特色退圖示卡。
- 截圖：`web/src/features/funnel/tour/{table,roomHost,roomGuest}.zh-CN.webp`，來自 2026-11-03 正式伺服器端到端實測（測試通道房主＋Playwright WebKit iPhone 13 兩支手機，免費模型）。

## 下一步
- 朋友實機測試：pCloud 包換成 0.5.2（作者上傳 `~/Downloads/TableTavern-v0.5.2/` 兩個檔）＋手機加入；回報問題再修。
- 截圖裡模型顯示仍是 default（拍攝早於「默认」改動），不重拍。

## 觀察
- 房間模式中桌面版旁白末尾出現「GM 请“玩家”发言」，左欄只把一名真人玩家的卡放進「你的角色卡」槽。
- 用帶配對碼的出額度者連結重開頁面會跳「配對碼不對或已過期」（配對碼一次性、已被認領）。
