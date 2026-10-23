# desktop-tour — 網頁版的桌面版導覽頁（圖文並茂、吸引下載）

已 squash 合進 main（2026-10-23）、分支已刪。計畫：[plans/desktop-tour.md](../plans/desktop-tour.md)（裁決、頁面結構、截圖規格、驗收）。

## 現況
- `#download` 從小對話窗擴成滿版導覽頁：固定頁首（唯一關閉鈕）、主視覺（主標、副標、下載鈕、版本行、主圖）、四張截圖特色卡＋兩張圖示卡、功能對照表、安裝繞過說明＋第二組下載鈕。程式在 `web/src/features/funnel/`（`DownloadPage.tsx`、`tour-images.ts`、`tour-features.ts`、`tour/*.webp`），樣式 `web/src/App.css` 的 `.tour-*`，十語系各加 16 鍵。
- 截圖 10 張（繁中、英文各 5）全是我方範例桌內容、零 AI；清單見下。
- 驗收：verify 12 步綠；Sol、Grok、opus-review 三方驗收通過（2026-10-23）；Chrome 實開 14 組（375×667 十語系＋無正式版、1280×800、深色）下載鈕在第一屏、無橫向溢出。
- WebKit 補跑（2026-10-23）：`npm run e2e` 全部通過、`npm run smoke:dist` 全部通過。
- **已上線**（2026-10-23）：作者在 Cloudflare Pages 對 main 7f686cb 手動部署，正式環境即該版。站上實開 `https://table-tavern.pages.dev/#download`：十張 `/assets/*.webp` 皆 200、`immutable` 長快取、1600×1136；1280×800 深色與 375×812 淺色都無橫向溢出、下載鈕在第一屏、版本 v0.3.0、下載連結指 v0.3.0 安裝檔。
- 本案已無剩餘事項。順手發現已立案：[worldbook-keys-separator-i18n](../tasks/worldbook-keys-separator-i18n.md)。

## 已知環境坑
`npx playwright install webkit`（Playwright 1.55.1）在 Node 26 上下載完會卡在解壓（只寫出 `libwebrtc.dylib` 就停、CPU 0%）。解法：停掉安裝，用系統 `unzip` 把暫存區的 `playwright-download-webkit-*.zip` 解到 `~/Library/Caches/ms-playwright/webkit-<rev>/`，再 `touch INSTALLATION_COMPLETE`。

## 截圖清單
全部 1600×1136（視窗內容實高 568pt；不留底帶、不置中裁），深色主題（app 預設），測試通道 fresh root、零 AI。台詞、狀態欄值、世界書條目、開桌描述皆我方自寫，以 `invoke` 寫入。

| 鍵 | 畫面 | 遮罩 |
|---|---|---|
| table.zh-TW | 範例桌遊玩：GM 旁白＋狐狸／騎士／吟遊詩人各一句 | 隱藏未連線提示（onboarding 面板 display:none） |
| table.en | 同上（Fox／Knight／Bard），頂部狀態列收合一行 | 隱藏未連線提示 |
| subscription.zh-TW | 設定 › AI 連線：OpenRouter＋四個 CLI 選項 | 四個 CLI「已偵測：版本」blur(10px)（本機 CLI 安裝狀態） |
| subscription.en | 同上 | 同上 |
| oneLine.zh-TW | 大廳「一句話開桌」對話窗，填寫描述未送出 | 隱藏背景大廳的未連線提示 |
| oneLine.en | 同上 | 同上 |
| status.zh-TW | 範例桌狀態欄展開（時間／地點／在場／天氣／金幣／好感樹）＋對話 | 隱藏未連線提示 |
| status.en | 同上 | 隱藏未連線提示 |
| editor.zh-TW | 世界設定：world.md＋世界書 3 條（GM 專有／1 位角色／全體公開） | 無 |
| editor.en | 同上 | 無 |

狀態欄要有狀態樹或「狀態欄」標記才會掛上：用 `set_state_path` 寫一支「好感」樹讓它出現。重拍方法：`npm run harness:build` 後照計畫第四節，原始 PNG 與寫入腳本當時留在 scratchpad，不進 repo。
