# macOS 權限彈窗掛我們的名字：查因與取捨

> 現場調查與拍板：2026-07-28。結論是「說明清楚」而不是「消除」，因為消除的代價更糟。

## 現象

玩家把對話或生圖來源設成 CLI（Claude Code 等）後，macOS 陸續跳出

> 「Table Tavern」想要取用你「桌面」檔案夾中的檔案。

音樂、下載項目、網路卷、文件都出現過。一個桌遊 app 要這些東西，看起來就像可疑軟體。

## 真因

不是我們的程式碼碰那些資料夾（我們給 CLI 的參數是 `--tools ""`，它連工具都沒有）。是 **Claude Code CLI 一啟動就自建 macOS 沙盒**，沙盒初始化時由系統代為索取標準資料夾權限；macOS 照「responsible process」歸戶，子行程預設繼承父行程，於是掛在我們頭上。

tccd 日誌實證（2026-07-28 14:00:28）：

```
accessing  = com.anthropic.claude-code  (~/.local/share/claude/versions/2.1.210)
requesting = com.apple.sandboxd
responsible= com.tabletavern.app
service    = kTCCServiceMediaLibrary / kTCCServiceSystemPolicyDesktopFolder
```

## 試過三條路

**一、叫 Claude Code 別建沙盒** — 從執行檔挖出未公開的 `CLAUDE_CODE_SANDBOXED=1` 並帶入子行程。無效，打包版實測仍索取媒體資料庫權限。已移除。

**二、把責任歸屬還給 CLI**（`responsibility_spawnattrs_setdisclaim`，posix_spawn 屬性）— 機制本身有效，實測子行程的責任人確實變成它自己。但**結果更糟**：Claude Code 是裸執行檔、檔名就是版本號，彈窗因此變成

> 「2.1.210」想要取用 Apple Music、你的音樂和影片活動和媒體資料庫。

沒有名字、沒有說明（裸執行檔沒有 Info.plist），比掛我們的名字更可疑。已回退（實作 `7da2b55`，回退 `f1aab1c`）。

**三、說明清楚**（採用）— 兩層：

- 系統彈窗附我們自己的說明句：`src-tauri/Info.plist` 六個 `NS*UsageDescription` 鍵，各語系翻譯在 `src-tauri/macos/<語言>.lproj/InfoPlist.strings`，由 `bundle.macOS.files` 帶進 app 包。彈窗語言跟**系統語言**走，不是 app 內的語言設定。加語言＝複製一個 `.lproj`、翻一行、`tauri.conf.json` 加一行。
- app 內先預告：每家 CLI 第一次啟用（設定頁按儲存、CLI 尚未被叫起來）時彈一次，說明頁常駐同一段，生圖對話框選到 CLI 來源時也顯示。

## 已知但不處理

**開發期每次重打包都重問**：ad-hoc 簽章下 macOS 記的是二進位雜湊，重編＝新身分，舊授權作廢（日誌關鍵字 `Failed to match existing code requirement`，一天內 22 次，全部對上打包時刻）。玩家拿固定版本只會被問一次。正式 Developer ID 簽章可根治，見 `release-1-mac-signing`。

**玩家按「不允許」完全不影響功能**，我們本來就不讀那些位置。

## 什麼時候該重看這個決定

- Claude Code 改成正常的 `.app` 包（有顯示名稱與說明文字）→ 路線二立刻變成最佳解，程式碼在 `7da2b55` 撿得回來。
- 若實測發現 codex／agy／grok 的彈窗名字是可辨識的執行檔名，可以只對那幾家啟用路線二（歸屬是每次啟動子行程各自決定，能單獨開關）。
- 拿到 Developer ID 之後，重新確認彈窗是否仍會反覆出現。

## 實作時踩的雷

自建管線接 posix_spawn 時，**管線必須設 close-on-exec**：否則子行程會連父行程手上的另外兩組管線一起繼承，它的 stdin 永遠等不到 EOF，整支卡死（現象：`cargo test` 逾時，子行程停在 `cat`）。dup2 到 0／1／2 的那三個會自動去掉這個旗標，所以六個 fd 全設即可。
