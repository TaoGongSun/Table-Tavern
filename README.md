# Table Tavern（桌面酒館）

桌面 App：多角色 AI 桌上角色扮演。每個角色一份獨立上下文與一張角色卡（公開＋私有設定），一位讀得到世界設定的 GM 導演負責旁白、點名與推進劇情——角色只知道 GM 說出口的內容，資訊邊界由敘事天然實現。

## 特色

- **多角色群聊**：每個角色獨立上下文、逐角色打字指示、點名發言。
- **GM 導演**：讀得到世界設定與全部角色卡，負責旁白與推進回合。
- **世界書**：SillyTavern 相容條目格式，一鍵匯入匯出；條目可指定角色可見，形成資訊邊界。
- **角色卡匯入**：直接吃 SillyTavern V2 角色卡 PNG，含角色圖顯示開關。
- **場景管理**：換幕、場景摘要、前幕歷史瀏覽、單幕匯出。
- **跑團紀錄匯出**：一鍵存成 Markdown，位置自選。
- **雙語介面**：繁體中文（台灣）／English，範例桌內容跟著語系走。
- **資料全在本機**：純 Markdown／JSONL，可自行備份或直接編輯。

## 平台

- **macOS**（Apple Silicon）：DMG，目前為 ad-hoc 簽章測試版（未公證）。
- **Windows**：未簽章測試包由 CI 產出（推 `test-v*` tag 或手動觸發），正式簽章發佈籌備中。

## 安裝（macOS）

1. 下載 `Table Tavern_x.y.z_aarch64.dmg`，雙擊掛載。
2. 把 `Table Tavern.app` 拖進「應用程式」資料夾。
3. 雙擊開啟。

### 第一次開啟被 Gatekeeper 擋下？

本 App 未上架、未經 Apple 公證（ad-hoc 簽章），第一次開啟會看到「Apple 無法驗證『Table Tavern』是否為惡意軟體」。兩種解法擇一：

- **系統設定**：在對話框按「完成」，開「系統設定 → 隱私權與安全性」，捲到頁面底部按「仍要打開」，再確認一次即可。（macOS 15 之後，右鍵「打開」已不再提供例外選項。）
- **終端機**：`xattr -cr "/Applications/Table Tavern.app"` 之後正常雙擊。

## 開通（二選一）

- **自備 API key（標準）**：首次開啟會直接落在內建範例桌，畫面上的引導會帶你註冊 [OpenRouter](https://openrouter.ai/)、儲值小額並貼上 key——一把 key 通吃多家模型，角色與 GM 可以用不同檔位。除 key 外沒有任何必填欄位。
- **官方 CLI 訂閱模式（進階）**：已有 Claude、ChatGPT、Google 或 Grok 訂閱的使用者，可在「AI 設定」改以官方 CLI（Claude Code／Codex／Gemini CLI／Grok CLI）為傳輸層；四家都提供一鍵安裝——App 開啟可見的終端機視窗跑官方安裝腳本並引導登入，全程不經手帳密與 token。注意：供應商條款禁止第三方工具使用訂閱憑證，啟用前 App 會顯示具體風險告知，後果由你自己的帳號承擔。

## 資料存放

- 桌（世界）、角色卡、對話紀錄：`~/Documents/TableTavern/worlds/`（純 Markdown／JSONL，可自行備份或編輯）
- 全域設定與 API key：`~/Library/Application Support/TableTavern/config.json`（檔案權限 0600）

## 開發

```bash
npm install
npm run tauri dev    # 開發模式
npm run tauri build  # 產出 .app 與 DMG（src-tauri/target/release/bundle/）
cd src-tauri && cargo test   # Rust 測試
npm run build        # 前端型別檢查＋建置
```

技術棧：Tauri 2（Rust）＋ Vite + React + TypeScript。產品規格見 `NewPlan.md`，工程起手見 `KICKOFF.md`，版本異動見 [CHANGELOG.md](CHANGELOG.md)。

## 授權

[AGPL-3.0-only](LICENSE)。
