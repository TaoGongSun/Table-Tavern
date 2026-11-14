# 待實測清單

只收**這台機器驗不到**的實測：測試通道驗得到的項目要在結案前測完，不排進來（規則見專案 CLAUDE.md「實測在結案前做完」）。每列寫「測不到原因」與「還沒測」的那一步；測完由測的人在同一筆 commit 把該列刪掉，不留已測標記。`npm run verify` 會檢查這份檔的格式。
2026-11-15 整理：這台機器測得到的項目已全部用測試通道測完並移除。

## 非作者本人不可

- [shell-update-flash](../handoffs/archive/shell-update-flash.md)｜測不到原因：測試通道的按鍵不觸發瀏覽器原生移焦，要有人用真鍵盤邊按邊看｜還沒測：送出後新殼在背面載入那段時間按 Tab／Shift+Tab，焦點不該進兩支 iframe。2026-10-28 主線用真鍵盤測過焦點沒進 iframe，但期間視窗短暫失焦 3 次、原因未定，要人判斷玩家感不感覺得到
- [codex-lane-resume](../handoffs/archive/codex-lane-resume.md)｜測不到原因：要作者自己的 codex 帳號登出再登入、換帳號｜還沒測：①沒登入時在設定頁按登入，登入完 app 發言能跑　②換帳號後 app 下一輪用新帳號、不出現 CodexAuthSlotOccupied　③換帳號後 `<config_root>/codex-home/auth.json` 仍是指向 `~/.codex/auth.json` 的 symlink
- [vn-audio](../handoffs/vn-audio.md)｜測不到原因：測試通道拿不到實際出聲的證據，要人耳聽｜還沒測：標題主題曲、換曲時 1 秒淡入淡出、靜音與音量滑桿、放開音效滑桿的試聽聲，聽起來對不對
- [room-status-strip](../handoffs/room-status-strip.md)｜測不到原因：提示音要人耳聽｜還沒測：桌面版房主的提示音在玩家送出、進房時有沒有響、會不會太吵
- [room-table-chat](../handoffs/room-table-chat.md)｜測不到原因：測試通道的合成按鍵沒有輸入法組字狀態｜還沒測：桌面房間聊天用中文輸入法選字時按 Enter，不會把還沒選完的字送出去
- [vn-mode](../handoffs/archive/vn-mode.md)｜測不到原因：測試通道模擬不了系統拖放，也看不到 Finder｜還沒測：把 `.ttvn` 從 Finder 拖進大廳能匯入；在 Finder 雙擊 `.ttvn` 不會被別的程式打開
- [release-2-ci-windows](../handoffs/release-2-ci-windows.md)｜測不到原因：要照說明繞過 macOS 的未簽章警告，系統安全設定不能代按｜還沒測：從發佈頁下載 Mac 版，照繞過步驟安裝後能開

## 要真手機

- [multiplayer-room](../handoffs/multiplayer-room.md)｜測不到原因：手機端只用 Playwright WebKit 模擬 iPhone 驗過，真機的鍵盤、觸控與內建瀏覽器模擬不出來｜還沒測：①iOS Safari、Android Chrome 各走一輪加入、選角、送出、收回、擲骰、開狀態抽屜，鍵盤彈出時版面不亂　②iOS 鍵盤開著點送出是否要點兩次　③手機 QQ、微信內建瀏覽器出簡中引導、照做能跳到外部瀏覽器
- [room-status-strip](../handoffs/room-status-strip.md)｜測不到原因：iOS 的自動播放限制與熄螢幕行為要真機｜還沒測：手機房主提示音在 iOS Safari 點過畫面後會不會響；手機熄螢幕回來，聊天未讀數還在
- [room-player-cards](../handoffs/archive/room-player-cards.md)｜測不到原因：相機照片格式與真機檔案選擇器模擬不出來｜還沒測：真手機匯入 PNG 卡、換圖時選相機照片（JPEG／HEIC 轉 PNG）、AI 生圖後按「用這張」
- [room-phone-gallery](../handoffs/archive/room-phone-gallery.md)｜測不到原因：雙指縮放只能用真機觸控｜還沒測：編卡「從圖庫選」點縮圖開裁切窗，單指拖動、雙指縮放、套用後存檔；上傳相機照片也走一次裁切

## 要 Windows

- [refactor-card-png-export](../handoffs/refactor-card-png-export.md)｜測不到原因：沒有 Windows 機｜還沒測：Windows Explorer 看數十 MB 大卡的縮圖與檔案大小顯示
- [desktop-csp](../handoffs/desktop-csp.md)｜測不到原因：沒有 Windows 機｜還沒測：Windows 打包版①卡片介面桌介面照常載出、console 無 CSP 違規　②方案「驗證計畫」第 4 項的沙盒探測，探測指令計數只剩宿主那一次
- [agy-conversation-isolation](../handoffs/archive/agy-conversation-isolation.md)｜測不到原因：沒有 Windows 機｜還沒測：agy 已登入的 Windows 上 app 發言一輪，對話落在 `<config_root>/agy-home` 而非 `%USERPROFILE%\.gemini`；登入是否與終端機共用
- [codex-lane-resume](../handoffs/archive/codex-lane-resume.md)｜測不到原因：沒有 Windows 機｜還沒測：codex 已登入的 Windows 上 session 落在 `<config_root>/codex-home\sessions`；`auth.json` 與使用者目錄那份是同一檔；終端機重新登入後 app 下一輪自動接回
- [cli-version-gate](../handoffs/archive/cli-version-gate.md)｜測不到原因：沒有 Windows 機｜還沒測：執行中的 exe 不能覆寫時更新失敗保住舊版、逾時殺整組、codex junction 切換、grok `.old` 改名、一鍵安裝帶已測版、登入視窗帶關閉自動更新變數
- [ui-redesign](../handoffs/ui-redesign.md)｜測不到原因：沒有 Windows 機｜還沒測：WebView2 連按兩次 Esc 對話窗不被繞過關閉；Windows 上的整體外觀
- [long-prompt-scene-hint](../handoffs/archive/long-prompt-scene-hint.md)｜測不到原因：沒有 Windows 機｜還沒測：claude、grok、agy 已登入的 Windows 上，超過 3 萬字的桌各送一輪都回得出話，`cli-prompts/` 呼叫後是空的
- [claude-1h-cache](../handoffs/archive/claude-1h-cache.md)｜測不到原因：沒有 Windows 機｜還沒測：Windows 上一小時快取、閒置過期後重建這兩項行為與 macOS 一致
- [multiplayer-room](../handoffs/multiplayer-room.md)｜測不到原因：沒有 Windows 與 Linux 機｜還沒測：Windows `cargo check` 與開房時防休眠；Linux `systemd-inhibit` 防休眠
- [release-2-ci-windows](../handoffs/release-2-ci-windows.md)｜測不到原因：沒有 Windows 機｜還沒測：從發佈頁下載 Windows 版，照繞過步驟安裝後能開

## 要外部條件

- [api-shared-lane](../handoffs/api-shared-lane.md)｜測不到原因：要有會回報快取用量的付費 API 模型金鑰，手上的金鑰額度是 0｜還沒測：API 路徑的快取成對測試（同角色／換角色 × 冷／暖）
- [refactor-survey-spans](../handoffs/refactor-survey-spans.md)｜測不到原因：要有額度的 API 金鑰才能用 API 模式跑重構｜還沒測：API 模式下沒設中階模型時重構退回用 GM 模型（連同 [refactor-dispatch](../handoffs/refactor-dispatch.md) 的 P8）
- [image-model-picker](../handoffs/image-model-picker.md)｜測不到原因：要有儲值的 OpenRouter 帳戶｜還沒測：付費生圖模型回 JPEG、WebP 或遠端網址時都存成 PNG；設定頁在付費金鑰下顯示生圖模型選單（連同 [image-model-free-tier-hide](../handoffs/image-model-free-tier-hide.md)）
- [vn-mode](../handoffs/archive/vn-mode.md)｜測不到原因：要有額度的 OpenRouter 金鑰｜還沒測：VN 立繪來源選 API，生一張定稿與一張表情差分，臉層對齊、無接縫
- [vn-audio](../handoffs/vn-audio.md)｜測不到原因：要 Freesound 金鑰才抓得到音效，測試環境沒有｜還沒測：`[sfx]` 格到達時放一次音效；快轉時音效不連發、停下那格照響；完結畫面與「音樂」分頁的音效署名（[vn-credits](../handoffs/vn-credits.md)）；有音效的桌匯出再匯入後音效還在（[vn-pack-opaque](../handoffs/vn-pack-opaque.md)）
- [claude-compat-endpoint](../handoffs/claude-compat-endpoint.md)｜測不到原因：要真的 Claude 相容端點網址與金鑰｜還沒測：填入後發言一輪能回話、串流與錯誤訊息正常
- [agy-conversation-isolation](../handoffs/archive/agy-conversation-isolation.md)｜測不到原因：要從未登入過 agy 的乾淨 macOS 機或獨立帳號，不能在作者機登出測｜還沒測：app 設定頁連結 agy、在跳出的終端機登入後 app 發言正常，終端機直接 `agy` 免再登入
- [agy-conversation-isolation](../handoffs/archive/agy-conversation-isolation.md)｜測不到原因：要等 agy 出新版｜還沒測：升版後對話仍落在 `config/agy-home/.gemini`、登入沿用；`agy --help` 若多了資料目錄旗標就改用旗標
- [codex-lane-resume](../handoffs/archive/codex-lane-resume.md)｜測不到原因：要等 codex 出新版｜還沒測：升版後精簡旗標不報錯、rollout 沒有新的中途 developer 訊息、`turn.completed.usage` 仍是累計值、resume 回原 thread id、存檔失敗警告的文字與形狀還在
- [desktop-update-detect](../handoffs/desktop-update-detect.md)｜測不到原因：要兩個真的發佈版才走得完〔作者裁決 2026-10-12：第二版發出時驗〕｜還沒測：偵測到新版、更新、回退、刪舊版，以及跨資料格式的回退
- [ui-redesign](../handoffs/ui-redesign.md)｜測不到原因：要有帶資料格式轉換的新版，或真的同時出錯｜還沒測：格式轉換更新窗；設定視窗被外部指定分頁時的齒輪紅點；換幕提醒、錯誤、狀態三者同時出現時的版面
- [stable-free-failover](../handoffs/archive/stable-free-failover.md)｜測不到原因：真上游連續失敗造不出來，只在假端點驗過｜還沒測：真模型連續兩次失敗後自動換模、同句重送成功，聊天室提示行與帳本對得上
- [claude-1h-cache](../handoffs/archive/claude-1h-cache.md)｜測不到原因：帳號沒開通超額用量；另一項要改作者家目錄的 Claude 設定｜還沒測：①訂閱用到超額時跳一次超額提示、文案對到該輪實得時效　②作者設定裡帶 `FORCE_PROMPT_CACHING_5M=1` 時續聊線是否被壓成 5 分鐘
- [test-harness](../handoffs/archive/test-harness.md)｜測不到原因：要真的重跑 CLI 安裝與登入流程，會動到作者機器上已裝好的 CLI｜還沒測：安裝與登入時 `ai-log` 有 `cli-probe:*` 與 `cli-setup-terminal:*` 各一筆
- [test-harness](../handoffs/archive/test-harness.md)｜測不到原因：要資料確實隔離的環境（獨立 macOS 帳號或虛擬機）｜還沒測：正式包帶 `TT_HARNESS_ROOT` 啟動不產生 harness.json、沒有開監聽
- [multiplayer-room](../handoffs/multiplayer-room.md)｜測不到原因：要牆內網路、Caddy 主機，或整晚不碰機器｜還沒測：牆內系統代理下 WebSocket 連得上；Node 自架伺服器走 Caddy 實跑兩回合；命令列通道無人值守一晚、撞額度時手機能切路線
- [room-scene-capacity](../handoffs/room-scene-capacity.md)｜測不到原因：要舊版桌面安裝包｜還沒測：舊版桌面執行端接新伺服器，不出幕容量提醒也不報錯
- [multiplayer-friends-test](../handoffs/multiplayer-friends-test.md)｜測不到原因：要朋友用自己的手機與網路實際玩｜還沒測：朋友照下載頁步驟裝好、進房玩一場，回報卡住的地方
- [i18n-more-languages](../handoffs/i18n-more-languages.md)｜測不到原因：要各語言的母語者｜還沒測：九個非繁中語言的譯文由母語者讀過（版面、殘留字、鍵名外露已用測試通道驗過十語系）
