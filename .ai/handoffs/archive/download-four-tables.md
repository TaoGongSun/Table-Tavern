# download-four-tables — 下載頁改以四種桌介紹（併入 room-guide-reshoot）

**已結案（2026-11-06）**：網頁版下載頁三語共用同一結構，以大廳四種開桌方式為「四扇門」，加上單一詳情面板（`#download/<new|oneLine|room|vn>`）；卡片介面放在「開新的一桌」面板下半；房間面板借說明書圖並連到說明書，說明書頁首可回 `#download/room`；簡中只差 pCloud 下載鈕與 AI 來源文案（魔搭／硅基流动）。說明書三語修字、每節一張圖。截圖三語全部重拍，示範角色雄獸人（迷霧酒館照用）。手機端語言選單加 `flex-shrink: 0`（避免被截成「繁體中」）。設計與拍板見 [方案檔](../../plans/download-four-tables.md)。

**部署**：推 main 後網頁版自動部署；party 手機端的 CSS 隨下次房間前端部署生效（拍照時以注入同一行 CSS 預覽）。

**已知限制**：說明書 G08、G10 的 GM 路線顯示 codex CLI（測試目錄沒有免金鑰的 API 路線）；en 的 oneLineOutline 只看得到一位登場角色；三語門名複製在網頁版字典，桌面版大廳改字時要同步。

**素材**：示範卡、頭像、拍照劇本在 `TestCards/download-tour/`（本機限定）。範圍外已立案 card-preview-markdown。
