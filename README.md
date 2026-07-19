# Table Tavern（桌面酒館）

macOS 桌面 App：多角色 AI 桌上角色扮演。每個角色一份獨立上下文與一張角色卡（公開＋私有設定），一位讀得到世界設定的 GM 導演負責旁白、點名與推進劇情——角色只知道 GM 說出口的內容，資訊邊界由敘事天然實現。

## 安裝

1. 下載 `Table Tavern_x.y.z_aarch64.dmg`，雙擊掛載。
2. 把 `Table Tavern.app` 拖進「應用程式」資料夾。
3. 雙擊開啟。

### 第一次開啟被 Gatekeeper 擋下？

本 App 未上架、未經 Apple 公證（ad-hoc 簽章），第一次開啟會看到「無法打開，因為它來自未識別的開發者」。兩種解法擇一：

- **右鍵開啟**：在「應用程式」中對 `Table Tavern.app` 按右鍵 →「打開」→ 再按「打開」。若新版 macOS 沒有出現「打開」按鈕，到「系統設定 → 隱私權與安全性」頁面底部按「仍要打開」。
- **終端機**：`xattr -cr "/Applications/Table Tavern.app"` 之後正常雙擊。

## 開通（二選一）

- **自備 API key（標準）**：首次開啟會直接落在內建範例桌，畫面上的引導會帶你註冊 [OpenRouter](https://openrouter.ai/)、儲值小額並貼上 key——一把 key 通吃多家模型，角色與 GM 可以用不同檔位。除 key 外沒有任何必填欄位。
- **官方 CLI 訂閱模式（進階）**：已自行安裝並登入 Claude Code／Codex CLI 的使用者，可在「AI 設定」啟用以 CLI 為傳輸層。注意：供應商條款禁止第三方工具使用訂閱憑證，啟用前 App 會顯示具體風險告知，後果由你自己的帳號承擔。App 只偵測既有 CLI，不代辦安裝與登入。

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

技術棧：Tauri 2（Rust）＋ Vite + React + TypeScript。產品規格見 `NewPlan.md`，工程起手見 `KICKOFF.md`。
