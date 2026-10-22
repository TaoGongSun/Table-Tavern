# desktop-tour — 網頁版的桌面版導覽頁（圖文並茂、吸引下載）

已 squash 合進 main（2026-10-23）、分支已刪。計畫：[plans/desktop-tour.md](../plans/desktop-tour.md)（裁決、頁面結構、截圖規格、驗收）。

## 現況
- `#download` 從小對話窗擴成滿版導覽頁：固定頁首（唯一關閉鈕）、主視覺（主標、副標、下載鈕、版本行、主圖）、四張截圖特色卡＋兩張圖示卡、功能對照表、安裝繞過說明＋第二組下載鈕。程式在 `web/src/features/funnel/`（`DownloadPage.tsx`、`tour-images.ts`、`tour-features.ts`、`tour/*.webp`），樣式 `web/src/App.css` 的 `.tour-*`，十語系各加 16 鍵。
- 截圖 10 張（繁中、英文各 5）全是我方範例桌內容、零 AI；清單見下。
- 驗收：verify 12 步綠；Sol、Grok、opus-review 三方驗收通過（2026-10-23）；Chrome 實開 14 組（375×667 十語系＋無正式版、1280×800、深色）下載鈕在第一屏、無橫向溢出。
- **做好了、待部署**。部署由作者在 Cloudflare Pages 儀表板對 main 手動建立部署（D12／D42；專案 `table-tavern`，步驟見 [plans/web-version.md](../plans/web-version.md)「部署與上線檢查」）。

## 剩下
1. **WebKit e2e 補跑**：本機沒有 Playwright WebKit（`~/Library/Caches/ms-playwright` 不存在，要下載、作者未授權）。作者同意後 `cd web && npx playwright install webkit`，再 `npm run e2e` 與 `npm run smoke:dist`。Chrome 代跑會在既有第 6 步「取消未完成回合」逾時（main 上同樣，與本案無關）。
2. 部署後站上實開 `https://table-tavern.pages.dev/#download`：十張圖載得出來（`/assets/*.webp`）、桌機與手機各看一次、深淺色各一次。
3. 順手發現已立案：[worldbook-keys-separator-i18n](../tasks/worldbook-keys-separator-i18n.md)。

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
