# Changelog

格式依 [Keep a Changelog](https://keepachangelog.com/zh-TW/1.1.0/)，版號依 [Semantic Versioning](https://semver.org/lang/zh-TW/)。0.3.0 是第一個對外正式版；0.1.0、0.2.0 為內部測試版。

## [Unreleased]

## [0.5.1] — 2026-11-06

### 這一版有什麼
視覺小說模式的改進：
- **開桌改成三步**：沒想法也能直接開桌，空白就交給 AI 想；可以先挑題材、章數和選項會不會改變結局；美術準備隨時可以開始，不用等畫完就能先進標題畫面看進度。
- **播放器更像一般視覺小說**：背景鋪滿畫面、對話框疊在下緣；點畫面、按空白鍵或捲滾輪往下讀，按住 Ctrl 快進、按 H 隱藏介面；Esc 或右鍵打開遊戲選單。
- **背景更有變化**：同一個地點換時段會套上清晨、黃昏、夜晚的色調；新增陰天、霧、雨。AI 一寫到換場景就提早開始畫背景。
- **美術一覽**：從標題畫面或遊戲選單打開，一次看所有立繪和背景，可以重畫、上傳自己的圖替換、重試畫失敗的。
- **大廳看得到劇本進度**：桌卡會顯示「劇本撰寫中…」，撰寫失敗也會標出來；撰寫還在進行時，同一桌不會重複送出。
- **內心獨白比較不會說破秘密**：要求 AI 別在獨白裡講出或暗示還沒揭露的事（AI 偶爾還是可能說溜）。
- 修正多角色時偶爾把「下一位：某某」當成台詞顯示；「定稿」改稱「立繪」。

### 下載
[![網頁版-點開就玩](https://img.shields.io/badge/網頁版-點開就玩-E8A33D?style=for-the-badge)](https://table-tavern.pages.dev)
[![下載-Windows](https://img.shields.io/badge/下載-Windows-0078D6?style=for-the-badge&logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiPjxyZWN0IHg9IjEiIHk9IjEiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48cmVjdCB4PSIxMi41IiB5PSIxIiB3aWR0aD0iMTAuNSIgaGVpZ2h0PSIxMC41Ii8+PHJlY3QgeD0iMSIgeT0iMTIuNSIgd2lkdGg9IjEwLjUiIGhlaWdodD0iMTAuNSIvPjxyZWN0IHg9IjEyLjUiIHk9IjEyLjUiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48L3N2Zz4=)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.5.1/TableTavern-Windows-Setup.exe)
[![下載-macOS](https://img.shields.io/badge/下載-macOS-000000?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.5.1/TableTavern-macOS.dmg)

- 網頁版：<https://table-tavern.pages.dev>，點開就玩，不用安裝。
- [Windows（64 位元）](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.5.1/TableTavern-Windows-Setup.exe)
- [macOS（僅 Apple Silicon，M 系列晶片；不支援 Intel Mac）](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.5.1/TableTavern-macOS.dmg)
- 下方其他檔案是 App 內更新用的，不用下載。

### 你的資料去哪裡
- **存在你電腦**：桌、角色卡、對話紀錄是「文件/TableTavern/」裡的純文字檔。設定與 API 金鑰存在系統的應用程式設定資料夾（未加密的設定檔；macOS 上限本人讀寫）。轉換格式前的桌備份放在「文件/TableTavern/」裡的隱藏資料夾；更新下載的安裝包存在本機 App 資料夾。透過 Codex、Gemini 命令列工具玩時，對話紀錄存在 App 自己的設定資料夾（Grok 本來就是），不混進你在終端機的紀錄；登入沿用你終端機那份。
- **對話送去哪**：從你的電腦直接送到你選的 AI 服務——OpenRouter、你自己填的 API 網址，或你安裝的官方命令列工具（由那支工具送到該公司）。送出的內容包含角色卡、世界設定、世界書條目、對話紀錄和你的輸入。AI 服務怎麼處理這些資料，以該服務的條款為準。
- **OpenRouter**：授權在瀏覽器的 openrouter.ai 完成，換到的金鑰只存本機。App 也會向 OpenRouter 查模型清單、排行與金鑰剩餘額度。
- **生圖**：送到你選的生圖來源（OpenRouter、你自己填的 API 網址，或命令列工具）；回傳網址的圖從該網址下載。
- **檢查更新**：開 App 時與每 24 小時向 GitHub 查一次新版，下載更新與回退檔也從 GitHub。GitHub 會看到你的 IP。設定裡可以關掉自動檢查。
- **卡片自帶的介面**：有些卡的介面會從卡作者指定的網站載入字型、圖片或程式（例如 jsDelivr），那些網站會看到你的 IP。卡片介面在隔離框裡執行，裡面的外部腳本讀不到你的金鑰，但讀得到這桌的聊天內容與卡片變數。
- **一鍵安裝與更新命令列工具**：從該公司官網下載官方安裝腳本。Claude、Codex、Grok 優先裝 App 測過的版本（下載不到就改裝最新版），Gemini 一律裝最新版；App 呼叫時會關掉工具的自動更新（你在終端機用不受影響）。
- **開房間時**：只有開房間才會用到作者架的房間伺服器（tt-room.pirahus.com）。房間裡的故事、大家的暱稱、公開的角色資料和頭像、每回合的行動，會經這台伺服器在房主電腦和朋友手機之間傳送，並保存在上面。角色卡的秘密設定和上傳的原圖只經過、不保存（原圖最多暫留 10 分鐘）。房間 30 天沒人用就會清掉；完整劇情一直都在房主的電腦上。不開房間就不會連到它。
- **沒有**：使用統計、廣告追蹤。單人遊玩不經過作者的伺服器。

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

### What's new
Visual novel mode improvements:
- **Setting up a table now takes three steps**: start even with no idea at all and let the AI come up with one; pick a genre, the number of chapters and whether choices can change the ending; art preparation can start any time, and you can go to the title screen without waiting for it to finish — it shows the progress there.
- **The player feels more like a regular visual novel**: the background fills the screen and the dialogue box sits along the bottom; click, press Space or scroll to read on, hold Ctrl to fast-forward, press H to hide the interface; Esc or right-click opens the game menu.
- **More varied backgrounds**: the same place gets dawn, dusk and night tints as time passes, plus overcast, fog and rain. Backgrounds start drawing as soon as the AI writes a scene change.
- **Art gallery**: open it from the title screen or the game menu to see every sprite and background at once — redraw, replace with your own upload, or retry ones that failed.
- **Story progress in the lobby**: a table card shows "Writing the script…" and marks a failed draft; while a script is still being written, the same table won't send the request again.
- **Inner monologue is less likely to spoil secrets**: the AI is told not to state or hint at things not yet revealed in a character's thoughts (it may still slip occasionally).
- Fixed "Next: …" occasionally showing up as a line of dialogue with several characters.

### Downloads
[![Play-in your browser](https://img.shields.io/badge/Play-in%20your%20browser-E8A33D?style=for-the-badge)](https://table-tavern.pages.dev)
[![Download-Windows](https://img.shields.io/badge/Download-Windows-0078D6?style=for-the-badge&logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiPjxyZWN0IHg9IjEiIHk9IjEiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48cmVjdCB4PSIxMi41IiB5PSIxIiB3aWR0aD0iMTAuNSIgaGVpZ2h0PSIxMC41Ii8+PHJlY3QgeD0iMSIgeT0iMTIuNSIgd2lkdGg9IjEwLjUiIGhlaWdodD0iMTAuNSIvPjxyZWN0IHg9IjEyLjUiIHk9IjEyLjUiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48L3N2Zz4=)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.5.1/TableTavern-Windows-Setup.exe)
[![Download-macOS](https://img.shields.io/badge/Download-macOS-000000?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.5.1/TableTavern-macOS.dmg)

- Web version: <https://table-tavern.pages.dev> — play right in your browser, nothing to install.
- [Windows (64-bit)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.5.1/TableTavern-Windows-Setup.exe)
- [macOS (Apple Silicon / M-series only; Intel Macs are not supported)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.5.1/TableTavern-macOS.dmg)
- The other files below are used by in-app updates; you don't need them.

### Where your data goes
- **On your computer**: tables, character cards and chat logs are plain-text files in "Documents/TableTavern/". Settings and API keys live in the system's app settings folder (an unencrypted settings file; on macOS readable only by you). Table backups made before a format conversion are kept in hidden folders inside "Documents/TableTavern/"; downloaded update installers are kept in the app's local data folder. When you play through the Codex or Gemini command-line tool, the app keeps those chat logs in its own settings folder (Grok already worked this way), separate from your terminal history; sign-in is shared with your terminal.
- **Where chats are sent**: straight from your computer to the AI service you choose: OpenRouter, an API URL you enter yourself, or an official command-line tool you installed (which sends to its own company). What gets sent includes character cards, world settings, lorebook entries, the chat log and your input. How the AI service handles it is governed by that service's terms.
- **OpenRouter**: authorization happens on openrouter.ai in your browser, and the resulting key is stored only on your computer. The app also asks OpenRouter for the model list, rankings and your key's remaining credit.
- **Image generation**: requests go to the image source you choose (OpenRouter, an API URL you enter yourself, or a command-line tool); images returned as a URL are downloaded from that URL.
- **Update checks**: on launch and every 24 hours the app asks GitHub for a new version, and update and rollback files are downloaded from GitHub. GitHub sees your IP address. You can turn automatic checks off in Settings.
- **Card interfaces**: some cards' interfaces load fonts, images or scripts from sites chosen by the card author (for example jsDelivr), and those sites see your IP address. Card interfaces run in an isolated frame; external scripts inside them cannot read your keys, but they can read this table's chat content and card variables.
- **One-click CLI install and update**: downloads the official install script from that company's website. For Claude, Codex and Grok the app prefers the version it was tested with (falling back to the latest if that can't be downloaded); Gemini always gets the latest. The app turns off the tool's auto-update when the app runs it (your own terminal use is unaffected).
- **When you open a room**: the author's room server (tt-room.pirahus.com) is used only when you open a room. The room's story, everyone's nicknames, public character info and portraits, and each round's actions pass through this server between the host's computer and friends' phones, and are kept there. Character cards' secret settings and uploaded original images only pass through and are not kept (originals stay at most 10 minutes). A room nobody has used for 30 days is deleted; the full story always stays on the host's computer. If you never open a room, the app never connects to it.
- **None of these**: no usage statistics, no ad tracking. Solo play never goes through an author-run server.

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

## [0.5.0] — 2026-11-04

### 這一版有什麼
- **開房間跟朋友一起玩**：在電腦上開房間，朋友用手機點邀請連結就能加入，不用裝 App。每人一個角色、各自行動；主持故事的 AI（GM）在房主的電腦上跑，用的是房主的 AI 帳號（也可以改用某位朋友的）。房主照常玩自己的角色，還能把暫時離開的朋友交給 AI 代打、收回或重來上一回合、暫停與重開房間；朋友在手機上可以自己建角色、生角色圖、重選一張。玩到一半的單人桌也能從桌卡選單「用這張桌開房」。[看玩法說明](https://table-tavern.pages.dev/#guide/room)
- **視覺小說模式**：大廳新增「視覺小說」。AI 一邊寫故事一邊畫背景和人物，你用選項決定劇情走向；有章節、結局、回頭改選、跳過看過的段落、存檔和標題畫面。做好的作品可以匯出成一個檔案（.ttvn）給別人玩；也支援帶視覺小說設定的角色卡。
- **故事裡不再冒出 GM 的狀態標記**：房間的故事頁頂端改成一行「時間 · 地點 · 在場」。
- **簡體中文玩家**：網頁版可以改用魔搭或矽基流動（貼上金鑰就能用），桌面版也能接矽基流動；下載頁多了 pCloud。
- 內建範例桌的角色調整；頂欄按鈕改叫「連接 AI 模型」；用 Antigravity 生圖時只要必要的權限。

### 下載
[![網頁版-點開就玩](https://img.shields.io/badge/網頁版-點開就玩-E8A33D?style=for-the-badge)](https://table-tavern.pages.dev)
[![下載-Windows](https://img.shields.io/badge/下載-Windows-0078D6?style=for-the-badge&logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiPjxyZWN0IHg9IjEiIHk9IjEiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48cmVjdCB4PSIxMi41IiB5PSIxIiB3aWR0aD0iMTAuNSIgaGVpZ2h0PSIxMC41Ii8+PHJlY3QgeD0iMSIgeT0iMTIuNSIgd2lkdGg9IjEwLjUiIGhlaWdodD0iMTAuNSIvPjxyZWN0IHg9IjEyLjUiIHk9IjEyLjUiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48L3N2Zz4=)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.5.0/TableTavern-Windows-Setup.exe)
[![下載-macOS](https://img.shields.io/badge/下載-macOS-000000?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.5.0/TableTavern-macOS.dmg)

- 網頁版：<https://table-tavern.pages.dev>，點開就玩，不用安裝。
- [Windows（64 位元）](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.5.0/TableTavern-Windows-Setup.exe)
- [macOS（僅 Apple Silicon，M 系列晶片；不支援 Intel Mac）](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.5.0/TableTavern-macOS.dmg)
- 下方其他檔案是 App 內更新用的，不用下載。

### 你的資料去哪裡
- **存在你電腦**：桌、角色卡、對話紀錄是「文件/TableTavern/」裡的純文字檔。設定與 API 金鑰存在系統的應用程式設定資料夾（未加密的設定檔；macOS 上限本人讀寫）。轉換格式前的桌備份放在「文件/TableTavern/」裡的隱藏資料夾；更新下載的安裝包存在本機 App 資料夾。透過 Codex、Gemini 命令列工具玩時，對話紀錄存在 App 自己的設定資料夾（Grok 本來就是），不混進你在終端機的紀錄；登入沿用你終端機那份。
- **對話送去哪**：從你的電腦直接送到你選的 AI 服務——OpenRouter、你自己填的 API 網址，或你安裝的官方命令列工具（由那支工具送到該公司）。送出的內容包含角色卡、世界設定、世界書條目、對話紀錄和你的輸入。AI 服務怎麼處理這些資料，以該服務的條款為準。
- **OpenRouter**：授權在瀏覽器的 openrouter.ai 完成，換到的金鑰只存本機。App 也會向 OpenRouter 查模型清單、排行與金鑰剩餘額度。
- **生圖**：送到你選的生圖來源（OpenRouter、你自己填的 API 網址，或命令列工具）；回傳網址的圖從該網址下載。
- **檢查更新**：開 App 時與每 24 小時向 GitHub 查一次新版，下載更新與回退檔也從 GitHub。GitHub 會看到你的 IP。設定裡可以關掉自動檢查。
- **卡片自帶的介面**：有些卡的介面會從卡作者指定的網站載入字型、圖片或程式（例如 jsDelivr），那些網站會看到你的 IP。卡片介面在隔離框裡執行，裡面的外部腳本讀不到你的金鑰，但讀得到這桌的聊天內容與卡片變數。
- **一鍵安裝與更新命令列工具**：從該公司官網下載官方安裝腳本。Claude、Codex、Grok 優先裝 App 測過的版本（下載不到就改裝最新版），Gemini 一律裝最新版；App 呼叫時會關掉工具的自動更新（你在終端機用不受影響）。
- **開房間時**：只有開房間才會用到作者架的房間伺服器（tt-room.pirahus.com）。房間裡的故事、大家的暱稱、公開的角色資料和頭像、每回合的行動，會經這台伺服器在房主電腦和朋友手機之間傳送，並保存在上面。角色卡的秘密設定和上傳的原圖只經過、不保存（原圖最多暫留 10 分鐘）。房間 30 天沒人用就會清掉；完整劇情一直都在房主的電腦上。不開房間就不會連到它。
- **沒有**：使用統計、廣告追蹤。單人遊玩不經過作者的伺服器。

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

### What's new
- **Open a room and play with friends**: open a room on your computer and friends join from their phone by tapping an invite link — nothing to install. Everyone plays their own character; the AI that runs the story (the GM) runs on the host's computer, using the host's AI account (or a chosen friend's). The host keeps playing their own character and can let the AI play for a friend who steps away, take back or redo the last round, and pause and reopen the room; friends can create a character on their phone, generate character art and pick a different one. A solo table you're already playing can become a room from its table menu ("Open a room with this table"). [Read the guide](https://table-tavern.pages.dev/#guide/room)
- **Visual novel mode**: a new "Visual Novel" entry in the lobby. The AI writes the story while drawing backgrounds and characters, and you steer it with choices; with chapters, endings, going back to choose again, skipping parts you've read, saves and a title screen. Export a finished work as a single file (.ttvn) for others to play; character cards with visual-novel settings are supported too.
- **No more GM status markup popping up in the story**: the room's story tab now shows one line at the top, "Time · Place · Present".
- **Simplified Chinese players**: the web version can use ModelScope or SiliconFlow (paste your key and go), the desktop app can use SiliconFlow, and the download page adds pCloud.
- Built-in sample table characters adjusted; the top-bar button is now "Connect an AI model"; Antigravity image generation asks only for the permissions it needs.

### Downloads
[![Play-in your browser](https://img.shields.io/badge/Play-in%20your%20browser-E8A33D?style=for-the-badge)](https://table-tavern.pages.dev)
[![Download-Windows](https://img.shields.io/badge/Download-Windows-0078D6?style=for-the-badge&logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiPjxyZWN0IHg9IjEiIHk9IjEiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48cmVjdCB4PSIxMi41IiB5PSIxIiB3aWR0aD0iMTAuNSIgaGVpZ2h0PSIxMC41Ii8+PHJlY3QgeD0iMSIgeT0iMTIuNSIgd2lkdGg9IjEwLjUiIGhlaWdodD0iMTAuNSIvPjxyZWN0IHg9IjEyLjUiIHk9IjEyLjUiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48L3N2Zz4=)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.5.0/TableTavern-Windows-Setup.exe)
[![Download-macOS](https://img.shields.io/badge/Download-macOS-000000?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.5.0/TableTavern-macOS.dmg)

- Web version: <https://table-tavern.pages.dev> — play right in your browser, nothing to install.
- [Windows (64-bit)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.5.0/TableTavern-Windows-Setup.exe)
- [macOS (Apple Silicon / M-series only; Intel Macs are not supported)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.5.0/TableTavern-macOS.dmg)
- The other files below are used by in-app updates; you don't need them.

### Where your data goes
- **On your computer**: tables, character cards and chat logs are plain-text files in "Documents/TableTavern/". Settings and API keys live in the system's app settings folder (an unencrypted settings file; on macOS readable only by you). Table backups made before a format conversion are kept in hidden folders inside "Documents/TableTavern/"; downloaded update installers are kept in the app's local data folder. When you play through the Codex or Gemini command-line tool, the app keeps those chat logs in its own settings folder (Grok already worked this way), separate from your terminal history; sign-in is shared with your terminal.
- **Where chats are sent**: straight from your computer to the AI service you choose: OpenRouter, an API URL you enter yourself, or an official command-line tool you installed (which sends to its own company). What gets sent includes character cards, world settings, lorebook entries, the chat log and your input. How the AI service handles it is governed by that service's terms.
- **OpenRouter**: authorization happens on openrouter.ai in your browser, and the resulting key is stored only on your computer. The app also asks OpenRouter for the model list, rankings and your key's remaining credit.
- **Image generation**: requests go to the image source you choose (OpenRouter, an API URL you enter yourself, or a command-line tool); images returned as a URL are downloaded from that URL.
- **Update checks**: on launch and every 24 hours the app asks GitHub for a new version, and update and rollback files are downloaded from GitHub. GitHub sees your IP address. You can turn automatic checks off in Settings.
- **Card interfaces**: some cards' interfaces load fonts, images or scripts from sites chosen by the card author (for example jsDelivr), and those sites see your IP address. Card interfaces run in an isolated frame; external scripts inside them cannot read your keys, but they can read this table's chat content and card variables.
- **One-click CLI install and update**: downloads the official install script from that company's website. For Claude, Codex and Grok the app prefers the version it was tested with (falling back to the latest if that can't be downloaded); Gemini always gets the latest. The app turns off the tool's auto-update when the app runs it (your own terminal use is unaffected).
- **When you open a room**: the author's room server (tt-room.pirahus.com) is used only when you open a room. The room's story, everyone's nicknames, public character info and portraits, and each round's actions pass through this server between the host's computer and friends' phones, and are kept there. Character cards' secret settings and uploaded original images only pass through and are not kept (originals stay at most 10 minutes). A room nobody has used for 30 days is deleted; the full story always stays on the host's computer. If you never open a room, the app never connects to it.
- **None of these**: no usage statistics, no ad tracking. Solo play never goes through an author-run server.

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


## [0.4.0] — 2026-10-29

### 這一版有什麼
- **第一次 App 內更新**：0.3.0 會在版本號旁看到更新提示，一鍵升上來；升級後不滿意，可以在 App 內回到 0.3.0。回到 0.3.0 時對話紀錄與狀態欄數值都在；但在 0.3.0 裡繼續玩或改卡，0.4.0 新增的設定會遺失，升回來也不會恢復：舞台卡標記要重新設定；用 0.4.0 重構過的桌會失去部分重構設定（例如 GM 可能又照原卡印出狀態欄），需要的話開新桌重構。
- **多角色桌想聽誰就點誰**：GM 旁白下方有「讓○○接話」，送出鍵旁有「送出並接話」。
- **舞台卡**：只提供設定、不會開口的卡（地點、勢力、系統說明），在角色卡「⋯」選單「設為舞台卡」；AI 重構也會自動判斷。
- **更多酒館卡照原樣玩**：支援 zod 規則的 MVU 變數卡（桌面與網頁版）；故事照酒館顯示卡片的 HTML 排版與文字樣式，不再露出 HTML 註解；只作用在提示詞的正則照酒館套用；開場白照酒館只當第一則訊息，不再把開場白重複放進角色設定。
- **卡片介面更順**：回合更新不再閃一下；新畫面載太久時保留原畫面；換幕／關閉工具列不再蓋住卡片。
- **AI 重構更穩**：同一欄有多份規則會合併、不再整次套用失敗；略過的規則與矛盾欄位列在結果裡；狀態欄每一格（含地圖、記憶摘要）都照常追蹤；人名裡的 {{user}} 換成你的名字；套用後按「復原上次匯入」不會連重構一起撤掉。
- **「收回上一句」按一次就收一則**，不用連按。
- **用訂閱的命令列工具**：一鍵安裝時，Claude、Codex、Grok 優先裝 App 測過的版本（下載不到就改裝最新版），Gemini 一律裝最新版；App 呼叫時不讓工具自己升版；工具升版後格式對不上會明說，並給「檢查 CLI 更新」鈕。Codex 續聊更省額度。
- 修正多個會漏記遊玩資料（狀態欄數值、一次性事件）的問題，以及換幕或分岔回來角色顯示不對、GM 重印已淘汰的狀態欄等問題。
- 桌面版主視窗加上更嚴格的安全限制。

### 下載
[![網頁版-點開就玩](https://img.shields.io/badge/網頁版-點開就玩-E8A33D?style=for-the-badge)](https://table-tavern.pages.dev)
[![下載-Windows](https://img.shields.io/badge/下載-Windows-0078D6?style=for-the-badge&logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiPjxyZWN0IHg9IjEiIHk9IjEiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48cmVjdCB4PSIxMi41IiB5PSIxIiB3aWR0aD0iMTAuNSIgaGVpZ2h0PSIxMC41Ii8+PHJlY3QgeD0iMSIgeT0iMTIuNSIgd2lkdGg9IjEwLjUiIGhlaWdodD0iMTAuNSIvPjxyZWN0IHg9IjEyLjUiIHk9IjEyLjUiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48L3N2Zz4=)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.4.0/TableTavern-Windows-Setup.exe)
[![下載-macOS](https://img.shields.io/badge/下載-macOS-000000?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.4.0/TableTavern-macOS.dmg)

- 網頁版：<https://table-tavern.pages.dev>，點開就玩，不用安裝。
- [Windows（64 位元）](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.4.0/TableTavern-Windows-Setup.exe)
- [macOS（僅 Apple Silicon，M 系列晶片；不支援 Intel Mac）](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.4.0/TableTavern-macOS.dmg)
- 下方其他檔案是 App 內更新用的，不用下載。

### 你的資料去哪裡
- **存在你電腦**：桌、角色卡、對話紀錄是「文件/TableTavern/」裡的純文字檔。設定與 API 金鑰存在系統的應用程式設定資料夾（未加密的設定檔；macOS 上限本人讀寫）。轉換格式前的桌備份放在「文件/TableTavern/」裡的隱藏資料夾；更新下載的安裝包存在本機 App 資料夾。透過 Codex、Gemini 命令列工具玩時，對話紀錄存在 App 自己的設定資料夾（Grok 本來就是），不混進你在終端機的紀錄；登入沿用你終端機那份。
- **對話送去哪**：從你的電腦直接送到你選的 AI 服務——OpenRouter、你自己填的 API 網址，或你安裝的官方命令列工具（由那支工具送到該公司）。送出的內容包含角色卡、世界設定、世界書條目、對話紀錄和你的輸入。AI 服務怎麼處理這些資料，以該服務的條款為準。
- **OpenRouter**：授權在瀏覽器的 openrouter.ai 完成，換到的金鑰只存本機。App 也會向 OpenRouter 查模型清單、排行與金鑰剩餘額度。
- **生圖**：送到你選的生圖來源（OpenRouter、你自己填的 API 網址，或命令列工具）；回傳網址的圖從該網址下載。
- **檢查更新**：開 App 時與每 24 小時向 GitHub 查一次新版，下載更新與回退檔也從 GitHub。GitHub 會看到你的 IP。設定裡可以關掉自動檢查。
- **卡片自帶的介面**：有些卡的介面會從卡作者指定的網站載入字型、圖片或程式（例如 jsDelivr），那些網站會看到你的 IP。卡片介面在隔離框裡執行，裡面的外部腳本讀不到你的金鑰，但讀得到這桌的聊天內容與卡片變數。
- **一鍵安裝與更新命令列工具**：從該公司官網下載官方安裝腳本。Claude、Codex、Grok 優先裝 App 測過的版本（下載不到就改裝最新版），Gemini 一律裝最新版；App 呼叫時會關掉工具的自動更新（你在終端機用不受影響）。
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

### What's new
- **First in-app update**: 0.3.0 shows an update marker next to the version number; update in one click, and roll back to 0.3.0 from inside the app if you'd rather not keep it. After rolling back, chat logs and status values are all there; but playing or editing cards in 0.3.0 drops settings added in 0.4.0, and updating again won't bring them back: stage-card marks need to be set again, and tables refactored in 0.4.0 lose part of their refactor setup (for example, the GM may start printing the card's original status bar again). If that matters, refactor on a new table.
- **Pick who speaks at multi-character tables**: "Let ○○ reply" appears under the GM's narration, and "Send + reply" sits next to the send button.
- **Stage cards**: cards that provide setting but never speak (places, factions, system notes). Use "Make Stage Card" in a character card's "⋯" menu; AI refactoring also detects them.
- **More SillyTavern cards play as-is**: MVU variable cards with zod rules (desktop and web); stories show a card's HTML layout and text styling like SillyTavern does, without leaking HTML comments; prompt-only regexes apply as in SillyTavern; the greeting is only the first message, as in SillyTavern, and is no longer repeated in the character's setup.
- **Smoother card interfaces**: no flash when a turn updates; if a new view takes too long to load, the old one stays; the scene-change/close toolbar no longer covers the card.
- **Steadier AI refactoring**: several rules for the same field are merged instead of failing the whole apply; skipped rules and conflicting fields are listed in the result; every status-bar field (including maps and memory summaries) is tracked; {{user}} in names becomes your name; "Undo Last Import" after applying no longer undoes the refactor too.
- **"Undo last" removes one message per click** — no more double clicks.
- **Subscription command-line tools**: one-click install now prefers the version the app was tested with for Claude, Codex and Grok (falling back to the latest if that version can't be downloaded) and always installs the latest Gemini; the tools can't update themselves while the app runs them; if a tool update breaks its output format, the app says so and offers "Check for CLI update". Codex follow-up turns use less of your quota.
- Fixed several cases where play data (status values, one-time events) went unrecorded, plus characters showing wrongly after scene changes or branching back, and the GM reprinting retired status bars.
- Stricter security limits on the desktop app's main window.

### Downloads
[![Play-in your browser](https://img.shields.io/badge/Play-in%20your%20browser-E8A33D?style=for-the-badge)](https://table-tavern.pages.dev)
[![Download-Windows](https://img.shields.io/badge/Download-Windows-0078D6?style=for-the-badge&logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiPjxyZWN0IHg9IjEiIHk9IjEiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48cmVjdCB4PSIxMi41IiB5PSIxIiB3aWR0aD0iMTAuNSIgaGVpZ2h0PSIxMC41Ii8+PHJlY3QgeD0iMSIgeT0iMTIuNSIgd2lkdGg9IjEwLjUiIGhlaWdodD0iMTAuNSIvPjxyZWN0IHg9IjEyLjUiIHk9IjEyLjUiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48L3N2Zz4=)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.4.0/TableTavern-Windows-Setup.exe)
[![Download-macOS](https://img.shields.io/badge/Download-macOS-000000?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.4.0/TableTavern-macOS.dmg)

- Web version: <https://table-tavern.pages.dev> — play right in your browser, nothing to install.
- [Windows (64-bit)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.4.0/TableTavern-Windows-Setup.exe)
- [macOS (Apple Silicon / M-series only; Intel Macs are not supported)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.4.0/TableTavern-macOS.dmg)
- The other files below are used by in-app updates; you don't need them.

### Where your data goes
- **On your computer**: tables, character cards and chat logs are plain-text files in "Documents/TableTavern/". Settings and API keys live in the system's app settings folder (an unencrypted settings file; on macOS readable only by you). Table backups made before a format conversion are kept in hidden folders inside "Documents/TableTavern/"; downloaded update installers are kept in the app's local data folder. When you play through the Codex or Gemini command-line tool, the app keeps those chat logs in its own settings folder (Grok already worked this way), separate from your terminal history; sign-in is shared with your terminal.
- **Where chats are sent**: straight from your computer to the AI service you choose: OpenRouter, an API URL you enter yourself, or an official command-line tool you installed (which sends to its own company). What gets sent includes character cards, world settings, lorebook entries, the chat log and your input. How the AI service handles it is governed by that service's terms.
- **OpenRouter**: authorization happens on openrouter.ai in your browser, and the resulting key is stored only on your computer. The app also asks OpenRouter for the model list, rankings and your key's remaining credit.
- **Image generation**: requests go to the image source you choose (OpenRouter, an API URL you enter yourself, or a command-line tool); images returned as a URL are downloaded from that URL.
- **Update checks**: on launch and every 24 hours the app asks GitHub for a new version, and update and rollback files are downloaded from GitHub. GitHub sees your IP address. You can turn automatic checks off in Settings.
- **Card interfaces**: some cards' interfaces load fonts, images or scripts from sites chosen by the card author (for example jsDelivr), and those sites see your IP address. Card interfaces run in an isolated frame; external scripts inside them cannot read your keys, but they can read this table's chat content and card variables.
- **One-click CLI install and update**: downloads the official install script from that company's website. For Claude, Codex and Grok the app prefers the version it was tested with (falling back to the latest if that can't be downloaded); Gemini always gets the latest. The app turns off the tool's auto-update when the app runs it (your own terminal use is unaffected).
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
[![網頁版-點開就玩](https://img.shields.io/badge/網頁版-點開就玩-E8A33D?style=for-the-badge)](https://table-tavern.pages.dev)
[![下載-Windows](https://img.shields.io/badge/下載-Windows-0078D6?style=for-the-badge&logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiPjxyZWN0IHg9IjEiIHk9IjEiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48cmVjdCB4PSIxMi41IiB5PSIxIiB3aWR0aD0iMTAuNSIgaGVpZ2h0PSIxMC41Ii8+PHJlY3QgeD0iMSIgeT0iMTIuNSIgd2lkdGg9IjEwLjUiIGhlaWdodD0iMTAuNSIvPjxyZWN0IHg9IjEyLjUiIHk9IjEyLjUiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48L3N2Zz4=)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.3.0/TableTavern-Windows-Setup.exe)
[![下載-macOS](https://img.shields.io/badge/下載-macOS-000000?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.3.0/TableTavern-macOS.dmg)

- 網頁版：<https://table-tavern.pages.dev>，點開就玩，不用安裝。
- [Windows（64 位元）](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.3.0/TableTavern-Windows-Setup.exe)
- [macOS（僅 Apple Silicon，M 系列晶片；不支援 Intel Mac）](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.3.0/TableTavern-macOS.dmg)
- 下方其他檔案是 App 內更新用的，不用下載。

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
[![Play-in your browser](https://img.shields.io/badge/Play-in%20your%20browser-E8A33D?style=for-the-badge)](https://table-tavern.pages.dev)
[![Download-Windows](https://img.shields.io/badge/Download-Windows-0078D6?style=for-the-badge&logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0id2hpdGUiPjxyZWN0IHg9IjEiIHk9IjEiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48cmVjdCB4PSIxMi41IiB5PSIxIiB3aWR0aD0iMTAuNSIgaGVpZ2h0PSIxMC41Ii8+PHJlY3QgeD0iMSIgeT0iMTIuNSIgd2lkdGg9IjEwLjUiIGhlaWdodD0iMTAuNSIvPjxyZWN0IHg9IjEyLjUiIHk9IjEyLjUiIHdpZHRoPSIxMC41IiBoZWlnaHQ9IjEwLjUiLz48L3N2Zz4=)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.3.0/TableTavern-Windows-Setup.exe)
[![Download-macOS](https://img.shields.io/badge/Download-macOS-000000?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.3.0/TableTavern-macOS.dmg)

- Web version: <https://table-tavern.pages.dev> — play right in your browser, nothing to install.
- [Windows (64-bit)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.3.0/TableTavern-Windows-Setup.exe)
- [macOS (Apple Silicon / M-series only; Intel Macs are not supported)](https://github.com/TaoGongSun/Table-Tavern/releases/download/v0.3.0/TableTavern-macOS.dmg)
- The other files below are used by in-app updates; you don't need them.

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
