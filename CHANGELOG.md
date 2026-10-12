# Changelog

格式依 [Keep a Changelog](https://keepachangelog.com/zh-TW/1.1.0/)，版號依 [Semantic Versioning](https://semver.org/lang/zh-TW/)。0.3.0 是第一個對外正式版；0.1.0、0.2.0 為內部測試版。

## [Unreleased]

## [0.3.0] — 2026-10-12（第一個桌面正式版）

AI 扮演一整桌角色，連 GM（主持人）也是 AI。丟一張角色卡就能直接聊，也能開一整桌：世界設定、世界書、換幕，由 GM 旁白並決定誰接話。每個角色只知道 GM 說出口的事，祕密在劇情揭露前就是祕密。

### 這一版有什麼
- **免費上手**：按「連接 OpenRouter」在瀏覽器授權就能玩，不必複製金鑰；App 自動挑免費模型，模型失效會自動換一支。想用付費模型再自己儲值，用多少付多少。
- **用你已有的訂閱**：Claude、ChatGPT、Gemini、Grok 的官方命令列工具可一鍵安裝並接上，花的是訂閱額度。各家對這種用法的政策不同，啟用前會顯示風險說明。
- **SillyTavern 卡直接用**：角色卡與世界書（PNG／JSON）匯入，世界書觸發照酒館規則；卡片自帶的介面（狀態面板、按鈕、變數）照常顯示和操作。
- **一句話開桌**，以及 **AI 重構角色卡**：把卡改成由 App 管格式、模型只報變化，省輸出（不保證每張卡都成功，難卡建議換高階模型）。
- 狀態欄面板、換幕與前情提要、跑團紀錄匯出 Markdown、卡片匯出 PNG。
- 網頁版的存檔可以匯入桌面版接著玩。
- 介面十國語言：繁中、简中、English、日本語、한국어、Español、Português (Brasil)、Deutsch、Français、Русский。
- **App 內更新**：之後有新版會在版本號旁提示，一鍵更新，也能回到上一版。這是第一個帶更新功能的版本，所以還沒有可以回去的舊版。

### 下載
- Windows（64 位元）：`TableTavern-Windows-Setup.exe`
- macOS（僅 Apple Silicon，M 系列晶片；不支援 Intel Mac）：`TableTavern-macOS.dmg`
- 帶版本號的 `.exe`／`.dmg` 是同一份檔案；`.sig`、`.app.tar.gz`、`latest.json` 是 App 內更新用的，不用下載。

### 你的資料去哪裡
- **存在你電腦**：桌、角色卡、對話紀錄是「文件/TableTavern/」裡的純文字檔。設定與 API 金鑰存在系統的應用程式設定資料夾（未加密的設定檔；macOS 上限本人讀寫）。轉換格式前的桌備份放在「文件/TableTavern/」裡的隱藏資料夾；更新下載的安裝包存在本機 App 資料夾。
- **對話送去哪**：從你的電腦直接送到你選的 AI 服務——OpenRouter、你自己填的 API 網址，或你安裝的官方命令列工具（由那支工具送到該公司）。送出的內容包含角色卡、世界設定、世界書條目、對話紀錄和你的輸入。AI 服務怎麼處理這些資料，以該服務的條款為準。
- **OpenRouter**：授權在瀏覽器的 openrouter.ai 完成，換到的金鑰只存本機。App 也會向 OpenRouter 查模型清單、排行與金鑰剩餘額度。
- **生圖**：送到你選的生圖來源（OpenRouter、你自己填的 API 網址，或命令列工具）；回傳網址的圖從該網址下載。
- **檢查更新**：開 App 時與每 24 小時向 GitHub 查一次新版，下載更新與回退檔也從 GitHub。GitHub 會看到你的 IP。設定裡可以關掉自動檢查。
- **卡片自帶的介面**：有些卡的介面會從卡作者指定的網站載入字型、圖片或程式（例如 jsDelivr），那些網站會看到你的 IP。卡片介面在隔離框裡執行，裡面的外部腳本讀不到你的金鑰，但讀得到這桌的聊天內容與卡片變數。
- **一鍵安裝命令列工具**：從該公司官網下載官方安裝腳本。
- **沒有**：作者的伺服器、使用統計、廣告追蹤。作者只看得到 GitHub 公開的下載次數。

### 安裝時被系統擋下？
桌面版還沒買程式碼簽章憑證、也還沒送 Apple 公證，所以系統會跳警告。請只從這個 GitHub 發佈頁下載。

**Windows**
1. 瀏覽器可能說這個檔案「不常下載」：Edge 點「…」→「保留」→「顯示更多」→「仍要保留」；Chrome 點「保留」。
2. 執行安裝檔，出現「Windows 已保護您的電腦」：點「其他資訊」→「仍要執行」。
3. 如果出現的是「智慧型應用程式控制已封鎖」而且沒有「仍要執行」：你的電腦開著智慧型應用程式控制，它不允許個別放行未簽章的程式，關掉之後要重灌 Windows 才能再開。不建議為此關閉——請先用網頁版，等之後的簽章版。
4. 少數 Windows 10 舊電腦沒有 WebView2，安裝檔會自動從微軟下載安裝。

**macOS**
1. 打開 DMG，把 Table Tavern 拖進「應用程式」。
2. 雙擊開啟。出現「Apple 無法驗證『Table Tavern』是否包含可能危害 Mac 或侵犯隱私的惡意軟體」：按「完成」（不要按「丟到垃圾桶」）。
3. 打開「系統設定 → 隱私權與安全性」，捲到最下面「安全性」，按「仍要打開」，用密碼或 Touch ID 確認，再按一次「打開」。這個按鈕要先試開過一次才會出現。
4. 只要做一次，之後正常雙擊就能開。

---

**English**

An AI plays a whole table of characters, including the GM. Drop in a single character card and start chatting, or set up a full table with world settings, a lorebook and scene changes, where the GM narrates and decides who speaks next. Each character only knows what the GM has said out loud, so secrets stay secret until the story reveals them.

### What's in this release
- **Free to start**: click "Connect OpenRouter", authorize in your browser, and play. No key to copy. The app picks a free model for you and switches to another if it stops working. Add credit to OpenRouter only if you want paid models; you pay for what you use.
- **Use the subscription you already have**: one-click install and hookup for the official Claude, ChatGPT, Gemini and Grok command-line tools, so play draws on your subscription. Each provider has its own policy on this kind of use; the app shows a risk notice before you turn it on.
- **SillyTavern cards just work**: import character cards and lorebooks (PNG/JSON); lorebook triggers follow SillyTavern's rules, and a card's built-in interface (status panels, buttons, variables) displays and works as usual.
- **One-sentence table setup**, plus **AI card refactoring**: the app takes over the card's fixed format and the model only reports what changed, which saves output. Not guaranteed to work on every card; for difficult cards, use a stronger model.
- Status panel, scene changes with recaps, session log export to Markdown, card export to PNG.
- Saves from the web version can be imported into the desktop app to keep playing.
- Ten interface languages: 繁體中文, 简体中文, English, 日本語, 한국어, Español, Português (Brasil), Deutsch, Français, Русский.
- **In-app updates**: when a new version is out, a marker appears next to the version number; update in one click, and roll back to the previous version if needed. This is the first version with updates built in, so there is no older version to roll back to yet.

### Downloads
- Windows (64-bit): `TableTavern-Windows-Setup.exe`
- macOS (Apple Silicon / M-series only; Intel Macs are not supported): `TableTavern-macOS.dmg`
- The versioned `.exe`/`.dmg` are the same files. `.sig`, `.app.tar.gz` and `latest.json` are used by in-app updates; you don't need them.

### Where your data goes
- **On your computer**: tables, character cards and chat logs are plain-text files in "Documents/TableTavern/". Settings and API keys live in the system's app settings folder (an unencrypted settings file; on macOS readable only by you). Table backups made before a format conversion are kept in hidden folders inside "Documents/TableTavern/"; downloaded update installers are kept in the app's local data folder.
- **Where chats are sent**: straight from your computer to the AI service you choose: OpenRouter, an API URL you enter yourself, or an official command-line tool you installed (which sends to its own company). What gets sent includes character cards, world settings, lorebook entries, the chat log and your input. How the AI service handles it is governed by that service's terms.
- **OpenRouter**: authorization happens on openrouter.ai in your browser, and the resulting key is stored only on your computer. The app also asks OpenRouter for the model list, rankings and your key's remaining credit.
- **Image generation**: requests go to the image source you choose (OpenRouter, an API URL you enter yourself, or a command-line tool); images returned as a URL are downloaded from that URL.
- **Update checks**: on launch and every 24 hours the app asks GitHub for a new version, and update and rollback files are downloaded from GitHub. GitHub sees your IP address. You can turn automatic checks off in Settings.
- **Card interfaces**: some cards' interfaces load fonts, images or scripts from sites chosen by the card author (for example jsDelivr), and those sites see your IP address. Card interfaces run in an isolated frame; external scripts inside them cannot read your keys, but they can read this table's chat content and card variables.
- **One-click CLI install**: downloads the official install script from that company's website.
- **None of these**: no author-run server, no usage statistics, no ad tracking. The only thing the author can see is GitHub's public download count.

### Blocked by your system during install?
The desktop app isn't code-signed or notarized by Apple yet, so your system will warn you. Only download it from this GitHub release page.

**Windows**
1. Your browser may say the file "isn't commonly downloaded": in Edge, click "…" → "Keep" → "Show more" → "Keep anyway"; in Chrome, click "Keep".
2. Run the installer. If you see "Windows protected your PC", click "More info" → "Run anyway".
3. If instead you see "Smart App Control blocked" with no "Run anyway": Smart App Control is on, and it doesn't allow exceptions for unsigned apps. Once turned off, it can only be turned back on by reinstalling Windows. We don't recommend turning it off for this; use the web version for now and wait for a signed release.
4. A few older Windows 10 PCs lack WebView2; the installer downloads it from Microsoft automatically.

**macOS**
1. Open the DMG and drag Table Tavern into Applications.
2. Double-click to open it. When you see "Apple could not verify 'Table Tavern' is free of malware that may harm your Mac or compromise your privacy", click "Done" (not "Move to Trash").
3. Open System Settings → Privacy & Security, scroll down to Security, click "Open Anyway", confirm with your password or Touch ID, then click "Open". The button only appears after you've tried to open the app once.
4. You only need to do this once; after that it opens normally.

## [0.2.0] — 2026-07-24（內部測試版）

### 新增
- UI 語系切換（繁體中文／English），角色與 GM 的語言規則依語系注入；範例桌內容跟著語系產生，首開先選語言。
- SillyTavern V2 角色卡匯入（PNG），含角色圖顯示／隱藏開關。
- 世界書 v2：SillyTavern 相容條目化、一鍵匯入匯出、條目可指定角色可見（資訊邊界）、恆定條目與 token 成本提示、條目置頂與排序。
- 場景管理：換幕與場景摘要、前幕歷史瀏覽、單幕匯出。
- 跑團紀錄一鍵匯出 Markdown（另存新檔對話框自選位置）。
- 角色卡隱藏區（軟刪除）＋還原＋真刪除確認。
- 設定視窗：單一入口內分頁（外觀：語言＋文字大小五檔／AI 連線）。
- CLI 訂閱供應商擴充：Gemini CLI（Antigravity）與 Grok CLI。
- CLI 一鍵安裝擴充到全部四家（Claude Code／Codex／Gemini CLI／Grok CLI）：可見終端機跑官方安裝腳本＋引導登入＋自動驗證連線。
- Windows 未簽章測試包 CI workflow（`test-v*` tag 或手動觸發）。

### 變更
- 版面重構：角色卡移左側欄、桌列表可摺疊、側欄寬度可拖曳。
- 編輯畫面按鈕列統一置頂（角色卡與世界設定一致）。
- 主欄閱讀優先版面：移除寬度上限、修掉常駐捲軸。
- App icon 換成自製圖示（啤酒杯＋角色卡），取代 Tauri 範本 logo。

### 修正
- CLI 一鍵安裝：Codex 驗證探針在非 git 目錄拒跑導致「驗證成功」訊息不出現，改用 `codex login status`。

## [0.1.0] — 2026-07-22（內部測試版）

首個可交付測試的 MVP。

### 新增
- 專案骨架：Tauri 2（Rust）＋ Vite + React + TypeScript。
- 資料層：世界／角色卡／對話紀錄／狀態持久化，純 Markdown＋JSONL 存於 `~/Documents/TableTavern/`。
- API 傳輸：OpenRouter SSE 串流，單角色對話。
- CLI 傳輸：Claude Code／Codex CLI headless 單發，含偵測與風險告知；檔位模型可覆寫、讀 CLI 本機模型快取。
- 群聊室 UI：桌側欄、三層級訊息視覺、逐角色打字指示、點名發言。
- 簡易導演（GM）：world.md＋全卡上下文、旁白、推進回合。
- Onboarding：首開內建範例桌，僅缺 key 時的 BYOK 引導。
- 打包：tauri build 產出 DMG（ad-hoc 簽章），Gatekeeper 繞行說明。
- 授權：AGPL-3.0-only。
