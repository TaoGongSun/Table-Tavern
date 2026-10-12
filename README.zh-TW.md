# Table Tavern（桌面酒館）

**[English →](README.md)**

AI 扮演一整桌角色，連 GM（主持人）也是 AI。丟一張角色卡進來就能直接聊；想玩深一點，就開一張完整的桌：世界設定、世界書、換幕，由 GM 旁白並決定誰接話。每個角色只知道 GM 在劇情裡說出口的事，祕密在揭露之前就真的是祕密，不靠 AI「假裝不知道」。

## 從 SillyTavern 來的？直接帶過來玩

- **角色卡、世界書直接匯入**：PNG 或 JSON 都行，你的卡也能再匯出帶走。
- **卡片自帶的介面和變數系統（MVU）照樣能玩**：卡裡寫好的狀態欄、按鈕、數值照樣出現、照樣會動。
- **網頁版的存檔可以帶進桌面版接著玩**，對話、變數、世界書狀態都跟著走。

## 馬上玩

[![網頁版-點開就玩](https://img.shields.io/badge/網頁版-點開就玩-E8A33D?style=for-the-badge)](https://table-tavern.pages.dev)
[![下載-Windows](https://img.shields.io/badge/下載-Windows-0078D6?style=for-the-badge&logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiPjxyZWN0IHg9IjEiIHk9IjEiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48cmVjdCB4PSIxMi41IiB5PSIxIiB3aWR0aD0iMTAuNSIgaGVpZ2h0PSIxMC41Ii8+PHJlY3QgeD0iMSIgeT0iMTIuNSIgd2lkdGg9IjEwLjUiIGhlaWdodD0iMTAuNSIvPjxyZWN0IHg9IjEyLjUiIHk9IjEyLjUiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48L3N2Zz4=)](https://github.com/TaoGongSun/Table-Tavern/releases/latest/download/TableTavern-Windows-Setup.exe)
[![下載-macOS](https://img.shields.io/badge/下載-macOS-000000?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/TaoGongSun/Table-Tavern/releases/latest/download/TableTavern-macOS.dmg)

**網頁版**：打開 <https://table-tavern.pages.dev> 就能玩，免費、不用安裝。網頁版一桌一張卡；玩得深了，想要多角色、GM、自己寫世界、用自己的訂閱，再下載桌面版。

網頁版的存檔放在你的瀏覽器裡。Safari 對很久沒打開的網站可能清掉資料，重要的存檔記得匯出。

## 安裝桌面版

點上面的下載按鈕就會直接下載最新版。正式版目前沒有做程式碼簽章，第一次開啟時系統會跳警告，照下面做一次就好，不是中毒。

**Windows**：執行下載的安裝檔。如果出現「Windows 已保護您的電腦」：點「其他資訊 → 仍要執行」。

**macOS**（Apple Silicon）：打開下載的 `.dmg`，把 `Table Tavern.app` 拖進「應用程式」，雙擊開啟。如果系統說無法驗證：按「完成」，打開「系統設定 → 隱私權與安全性」，捲到最底下按「強制打開」，再確認一次。

之後有新版，App 會自己提醒，按一下就更新。

## 開始玩

1. 第一次開啟會直接進到範例桌，隨便逛。
2. 按「連接 OpenRouter」（OpenRouter 是一個能接上很多家 AI 的服務），在瀏覽器登入或註冊、按授權，就自動回到桌上。不用複製貼上任何金鑰。
3. 開聊。不想自己寫卡？用一句話描述想玩的故事，App 幫你開一整桌。

## 為什麼好玩又省

- **可以完全免費玩**：連上 OpenRouter 後，App 自動挑一支目前好用的免費模型；那支忙線或暫時不能用，就自動換下一支，聊天室裡會告訴你換了。
- **已經有訂閱就直接用**：有 Claude、ChatGPT、Gemini 或 Grok 的訂閱，可以改走該家的官方命令列工具，花你本來就有的額度。App 內一鍵安裝並引導登入，偵測到登入成功就顯示「已連結」。各家對這種用法的規定不同，啟用前 App 會先說明風險，由你決定。
- **處處省額度**：
  - 盡量讓 AI 服務的「提示詞快取」命中，每輪重複送的設定不用一直全價重算；用量頁看得到命中率。
  - 卡片重構：會每輪重印整個介面的卡，整理過後介面由 App 自己畫，模型只回報有變動的數值，輸出短、等得也短。
  - 換幕時把前面的劇情壓成摘要帶進下一幕，不必一直背著整段對話。
  - GM 和每個角色可以各用不同等級的模型，重要的用好的、跑龍套的用便宜的。
  - 模型失控狂吐字會自動停下，回應也能隨時手動中止。
- **AI 生成角色圖**：替角色卡生一張角色圖。需要 OpenRouter 付費模型，或 ChatGPT／Gemini／Grok 的官方命令列工具；免費層不能生圖。
- **資料留在你的電腦**：桌、卡、對話紀錄都是本機檔案，對話直接從你的電腦送到你選的 AI 服務，中間沒有我們的伺服器。
- **十國語言介面**：繁中、简中、英、日、韓、西、葡（巴西）、德、法、俄，範例桌也跟著你的語言走。

## 常見問題

**要錢嗎？**
App 免費、開源，也可以全程用免費模型玩。想用更強的付費模型，再幫 OpenRouter 帳號儲值，用多少付多少，直接付給 AI 服務，沒有月費。

**可以贊助嗎？**
可以。在 Ko-fi 贊助會拿到贊助包，匯入後解鎖五套額外配色。所有功能不贊助也都能用。

**角色會不會偷看到不該知道的設定？**
不會。只有 GM 讀得到世界設定和每個人的卡，角色要等 GM 在劇情裡說出來才知道；世界書條目也能指定只給某些角色看。

**資料存在哪？怎麼備份？**
全部在「文件/TableTavern/」資料夾，整個複製起來就是備份。設定與金鑰另外放在系統的應用程式設定資料夾。

**有問題或建議？**
歡迎丟到 [GitHub Issues](https://github.com/TaoGongSun/Table-Tavern/issues)。

## 給工程師

桌面版是 Tauri 2（Rust）＋ React／TypeScript；網頁版在 `web/`，是另一個 Vite 專案。架構見 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)、檔案放哪見 [docs/STRUCTURE.md](docs/STRUCTURE.md)，版本異動見 [CHANGELOG.md](CHANGELOG.md)，歷次發版在 [GitHub Releases](https://github.com/TaoGongSun/Table-Tavern/releases)。

```bash
npm install
npm run tauri dev    # 桌面版開發模式
npm run tauri build  # 產出安裝檔／.app／DMG
npm run verify       # 一次驗完建置、測試、i18n、結構檢查
cd web && npm install && npm run dev   # 網頁版開發模式
```

## 授權

[AGPL-3.0-only](LICENSE)。
