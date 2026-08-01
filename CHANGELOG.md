# Changelog

格式依 [Keep a Changelog](https://keepachangelog.com/zh-TW/1.1.0/)，版號依 [Semantic Versioning](https://semver.org/lang/zh-TW/)。尚未正式對外發佈，以下皆為內部測試版。

## [Unreleased]

### 新增
- 介面擴充到十國語言（繁體中文、简体中文、English、日本語、한국어、Español、Português (Brasil)、Deutsch、Français、Русский）：介面字典、首開範例桌內容、AI 輸出語言規範三處同步上齊。
- `npm run check:i18n`：語系字典體檢（佔位符一致性＋按鈕文字寬度），改文案或加語系後跑。
- 收回上一句：一次收掉紀錄最後一則、可連按往回收（只收當前這一幕）；手滑按到可從訊息串底部的「復原」把最後收掉的那句放回去。

### 變更
- 語系字典拆成每語言一檔（`src/i18n/`），範例桌內容抽成資料檔（`src-tauri/samples/`）：新增語系不必動程式邏輯。
- 切換語言時同步更新網頁語言標記，中日韓字形與斷行才會正確。

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
