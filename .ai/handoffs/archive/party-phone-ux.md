# party-phone-ux — 多人房間手機網頁改版

**已結案（2026-11-06）**：多人房間手機網頁（`party/`）改版，解玩家回饋「劇情文字太小」（根因是上下固定列吃掉一半以上高度、開鍵盤後劇情歸零）。上方合一列、場景列一行、聊天式輸入格；打字時讓位、往回捲收起，也可手動收合（細列寫行動／草稿，打字中與倒數 30 秒不收、收起時按鈕第一下照常）；名單頁有顯示設定（房號、語系、劇情字級，字級存 `tt-party:story-size`）；配對碼收起。方案與拍板見 [方案檔](../../plans/party-phone-ux.md)。

**結構**：手機樣式在 `party/src/features/room/phone-layout.css`（只有手機外殼 `App.tsx` 載入）；桌面嵌入只多一層 `display: contents` 的 `.room-screen`，版面不變。顯示設定在 `party/src/features/display/`。

**部署**：推 main 後 party 前端隨下次房間部署生效。
