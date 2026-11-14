# 實測佇列（建議順序）

2026-11-14 清理：已過的項目移除，能用測試通道代測的已代測並寫回各列。只收「施工已完成，只剩可執行環境／使用者實機／外部條件驗收」的項目；待實作與待拍板不在此。
各項的驗收細目在各自任務檔與 `handoffs/<id>.md`，此處只排順序與理由。

## 梯 1：本地操作，不花 API 額度

4e. [refactor-card-png-export](../handoffs/refactor-card-png-export.md)：剩 Windows Explorer 看 #3 大卡（數十 MB）的縮圖與檔案大小顯示（無 Windows 環境）。

4g. [shell-update-flash](../handoffs/archive/shell-update-flash.md) 等待期鍵盤：送出後新殼在背面載的那段時間，用原生 Tab／Shift+Tab（真鍵盤，測試通道的 press 不觸發原生移焦）焦點進不了兩支 iframe 內的控制項。 2026-10-28 主線真鍵盤（isTrusted）測：卡住模式鎖定期按 Tab／Shift+Tab，焦點始終在宿主（rail 按鈕／BODY），從沒落到 IFRAME，兩支 iframe 都是 inert、tabIndex -1；但鎖定期間 Tab 後出現 3 次 window blur 隨即 focusin 回宿主（鎖定前也有 1 次），原因未定，不記為過（原始紀錄 `vq/4g-mainline.json`）。

4i. [desktop-csp](../handoffs/desktop-csp.md) Windows 安全驗收（macOS 已用測試通道代測；零 AI 派送）：Windows 打包版①卡片介面桌（WestFantsy）介面照常載出、DevTools console 無 CSP 違規　②方案「驗證計畫」第 4 項的沙盒探測（內層試 `__TAURI_INTERNALS__.invoke`、`fetch(http://ipc.localhost/…)`、`window.ipc.postMessage`／`chrome.webview.postMessage` 帶真 key）：探測指令計數只剩宿主那一次。

排這梯前先確認該項驗收步驟裡沒有換幕：換幕一定走模型產前情提要摘要（`advance_scene`），避不開。

- [lobby-table-badges](../handoffs/lobby-table-badges.md)（作者實機）：開一間房後回大廳，該桌排第一、封面帶「房間中・房間碼」橘色帶；桌數湊到 8 張（且聊天與 VN 都有）看篩選籤出現、切籤只隱藏不符的卡。 2026-11-14 測試通道代測：湊到 8 桌（聊天與 VN 都有）出現篩選籤、切籤計數正確；剩開房後排第一與橘色帶（要真開房）。
- [room-host-actions](../handoffs/archive/room-host-actions.md) 包 3（作者實機）：關房後單人玩到 GM 替朋友的卡說台詞，重開房間，看手機故事串上那段「朋友的卡被 GM 代演」顯示成什麼樣子（照實送成 NPC 段，靠「房主在關房期間自己玩了這一段」分隔辨認）。

## 梯 2：要開 API 實聊、會燒額度

- [vn-sprite-exit](../handoffs/vn-sprite-exit.md)：真模型同地點讓角色離開一輪，看 GM 是否照標 `[exit: 角色名]`、位置在最後台詞之後與離場旁白之前。 2026-11-14 測試通道代測：Haiku 一次沒標 `[exit]`（只用旁白寫離開）；Sonnet 以上未驗，仍待測。

| 順位 | 項目 | 為何排這個位置 |
|---|---|---|
| 14 | [api-shared-lane](../handoffs/api-shared-lane.md) | CLI 三路與 API 角色辨識已過。還缺：api 那一路的快取成對測試（同角色／換角色 × 冷／暖），要有回報快取的付費模型才量得到 |
| 19 | [refactor-statusbar-skeleton](../handoffs/archive/refactor-statusbar-skeleton.md) 狀態欄骨架桌 | 剩一項：生成中按「貼開場」等排隊時再按停止生成。原③④「生成中套用重構」驗不到（已有遊玩紀錄的桌拒收重構），不再列 |
| [agy-conversation-isolation](../handoffs/archive/agy-conversation-isolation.md) 首次登入（原標擋發佈；〔作者裁決 2026-10-29〕0.4.0 放行、列已知限制） | 要從未登入過 agy 的乾淨 macOS 機（或獨立帳號）：app 設定頁連結 agy、在跳出的終端機完成登入 → app 發言正常，且終端機直接 `agy` 免再登入（token 經假 HOME 的 keychain symlink 寫進真 keychain）。不在作者機登出測 |
| [agy-conversation-isolation](../handoffs/archive/agy-conversation-isolation.md) Windows | 等有 Windows 機且 agy 已登入：app 發言一輪，對話落在 `<config_root>/agy-home` 而非 `%USERPROFILE%\.gemini`（`%AppData%` 也看一眼，只回新檔筆數）；登入是否與終端機共用（不共用＝app 另行登入，可接受但要記）。落點不對就只對 agy 子程序加設 `APPDATA` 再重驗 |
| [agy-conversation-isolation](../handoffs/archive/agy-conversation-isolation.md) agy 升版 | 每次 agy 升版：重驗假 HOME 落點（測試通道發言後對話在 `config/agy-home/.gemini`）與登入沿用；看 `agy --help` 有沒有資料目錄旗標，有就改用旗標、拿掉 keychain symlink |
| [codex-lane-resume](../handoffs/archive/codex-lane-resume.md) macOS 設定頁登入（作者在場） | 本機真 app、作者自己操作 codex 帳號：①沒登入時在設定頁按登入，登入腳本正常啟動，登入完成後 app 發言能跑　②換帳號後，app 下一輪用的是新帳號，且不出現 CodexAuthSlotOccupied　③換帳號後 `<config_root>/codex-home/auth.json` 仍是指向 `~/.codex/auth.json` 的 symlink |
| [codex-lane-resume](../handoffs/archive/codex-lane-resume.md) Windows | 等有 Windows 機且 codex 已登入：app 發言一輪，session 落在 `<config_root>/codex-home\sessions` 而非 `%USERPROFILE%\.codex`（只回新檔筆數）；`codex-home\auth.json` 與 `%USERPROFILE%\.codex\auth.json` 是同一檔（硬連結）；終端機 `codex logout`＋`codex login` 後 app 下一輪自動接回、不需重登 |
| [codex-lane-resume](../handoffs/archive/codex-lane-resume.md) codex 升版 | 每次 codex 升版：重驗精簡旗標不報錯、rollout 沒有中途 `<skills_instructions>` 等新 developer 訊息、`turn.completed.usage` 仍是累計值、`thread.started` 在 resume 時回原 id；存檔失敗警告「Failed to save the conversation transcript」的文字與 `item.completed`／`error` 形狀還在（`cli::stream::codex_rollout_save_failed` 靠它判斷寬限殺掉後要不要丟線）；看有沒有只搬 sessions 的設定 |
| 24 | [undo-two-clicks-summary-line](../handoffs/archive/undo-two-clicks-summary-line.md) 收回一下收一樓 | 2026-11-14 測試通道代測：MVU 桌收回一下旁白與摘要行一起消失、數值回到玩家句那一刻，復原照原序回來；重構桌數值跟著收回復原。剩登場塊（這次沒產生）。 原驗法：測試通道、低階模型：MVU 桌（zod 卡）與樹模式重構桌各跑一回合讓 GM 改文字欄（出變動摘要行），多角色桌另跑一回合帶出兩張卡登場（私設、登場交錯）。「收回上一句」按一下：登場塊、GM 旁白、摘要、點名一起消失，玩家句留著，狀態欄數值與平欄（時間、在場）回到玩家句那一刻；「復原剛收回的」按一下整樓照原序回來、數值回到回合後；連按收回／復原逐樓往返 |
| 29 | [cli-version-gate](../handoffs/archive/cli-version-gate.md) 更新鈕（Windows） | Windows 實機：執行中的 exe 不能覆寫時更新失敗要保住舊版、Job Object 逾時殺整組、codex `current` junction 切換、grok install.ps1 `.old` 改名、一鍵安裝帶已測版（install.ps1 `$Target`／`CODEX_RELEASE`／`GROK_VERSION`）；登入視窗帶關閉自動更新變數 |
| 30 | [vn-mode](../handoffs/archive/vn-mode.md) VN 模式作者實機試玩（含 [vn-ui-polish](../handoffs/vn-ui-polish.md) 播放器改版、[vn-setup-wizard](../handoffs/vn-setup-wizard.md) 開桌精靈三步／自動定稿／色調遮罩／美術一覽） | 2026-11-14 測試通道代測：「匯入 VN 作品」鈕匯入、選「當可續寫的桌」後 Haiku 接寫 3 拍，口吻與背景接得上。剩拖放匯入與作者試玩。 原驗法：2026-11-04 已合 main（測試通道冒煙：VN 精靈到開演一輪過）、2026-11-05 播放器 UI／UX 改版合 main（點畫面／Space 推進、Esc／右鍵選單、快捷列在對話框下、標題「開始」從幕首）、2026-11-06 開桌精靈改版合 main（試玩重點見其交接檔「結案狀態」）；從沒實測過的是把 `.ttvn` 拖進大廳匯入、匯入選「當可續寫的桌」後讓 AI 接寫幾拍。試玩重點與現成測試包見交接檔「下一步」，各包已知限制見各方案檔 |
| 31 | [table-status-panel](../handoffs/table-status-panel.md) 通用狀態欄作者實機 | 2026-11-14 測試通道代測：桌面過（頂端一行、右側欄、改值後 GM 與角色照新值演）。剩手機開房抽屜與真機。 原驗法：空桌或一句話開桌跑一輪真旁白：頂端長出「時間｜地點｜在場」一行，點開右側欄有每張卡一格（狀態一句、持有物 chip）與追蹤三欄；改 status、刪 chip、「＋欄位」加一欄再下一輪，GM 照新值演；手機開房：故事頁一行三鍵點開底部抽屜看到同一份。截圖已主線親看（種樹），差真旁白與真機 |
| 32 | [gm-relay-unify](../handoffs/gm-relay-unify.md) GM 接力合一作者實機 | 2026-11-14 測試通道代測：單人過（讓 X 接話、送出並接話、推進、推進中停止、請他發言）；「推進到點名你就停」沒自然出現。剩房間雙端。 原驗法：單人：一般送出出「讓 X 接話」→點了接一句；送出並接話一句就停；GM 推進到點名你就停；推進中按停止（半截旁白標中斷、不再接）；請 X 發言照常。房間：開房雙端各推進一輪＋停止，手機逐字稿與桌面一致。測試通道截圖已主線親看，差真機與房間 |
| 33 | [ttrpg-rules-system](../handoffs/ttrpg-rules-system.md) 跑團 v1 作者實機 | 2026-11-14 測試通道代測：單人 Haiku 過（開擲骰、加 5e 多 51 條、骰號照序、空白擲完 GM 接、有草稿等送出）。剩「GM 寫 X，實際是 Y」、含骰子記號世界書的詢問、房間。 原驗法：單人聊天桌：桌設定開擲骰（對話出說明）→加入 5e SRD 5.1（世界書多 51 條、標籤與已加入）→跑一場戰鬥：GM 旁白帶 `(d20#N=V)`、骰池列標「GM 用了」、報錯顯示「GM 寫 X，實際是 Y」；GM 喊「請擲 1d20+3」時骰鈕出一鍵選項，空白擲完 GM 接著敘事、有草稿則等送出；匯入含骰子記號的世界書會問要不要開。房間：GM 照骰池代擲、手機看得到骰池列。測試通道截圖已主線親看（零 AI 造旁白），差真模型照不照序用骰與真機 |
| 34 | [ttrpg-combat-tracker](../handoffs/ttrpg-combat-tracker.md) 跑團戰鬥層作者實機 | 2026-11-14 測試通道代測：codex 低階模型主流程過（先攻、輪到誰、一鍵骰、增援、補血、戰鬥結束、悔棋還原）；⚠ 由 Haiku 自然觸發一次。剩點開 ⚠ 手改、依骰面改套灰字、換幕後、房間手機。 原驗法：單人預設桌開擲骰跟真模型打一場：GM 排先攻、扣血帶 dice 由 app 加減、收合列橘色「輪到 X」、收場清空先攻出「戰鬥結束」一行；模型報錯（沒排先攻加敵人、總和不符、非法 HP）面板不動、⚠ 白話原因、點開可手改、依骰面改套灰字；悔棋／還原／換幕後狀態正確；房間手機抽屜看得到輪到誰、敵人列、角色 HP。測試通道與手機預覽截圖已主線親看；Sonnet 真模型四場已驗過（結果見交接檔），差其他模型、⚠ 由真模型觸發與真機 |
| 36 | [sprite-matting-defringe](../handoffs/sprite-matting-defringe.md) 去背邊緣作者實機 | 2026-11-14 測試通道代測：agy 生一張黑短髮角色，中灰舞台背景放大看沒有淺邊；深色背景的舞台沒遇到。剩作者自己看。 原驗法：用 agy、API 或 Grok 當來源重新生成一個深色頭髮的角色，在深色背景的 VN 舞台上看髮梢與毛邊：沒有淺邊也沒有明顯黑邊；可併第 35 項一起看 |

## 梯 3：等外部條件，不排時程

機會來了順手做，不佔排程。

| 項目 | 卡在哪 |
|---|---|
| [refactor-survey-spans](../handoffs/refactor-survey-spans.md) T4 ② ＋ [refactor-dispatch](../handoffs/refactor-dispatch.md) P8 | 要真的用 API 模式跑一次才看得到 jsonl lane；CLI 模式測不到 |
| [stream-failure-visible](../plans/stream-failure-visible.md) T3–T5、T7 | 失敗態碰運氣重現，遇到再照計畫檔逐項核對 |
| [i18n-more-languages](../handoffs/i18n-more-languages.md) | 等網頁版與卡片匯出（refactor-card-png-export）都做完後一起驗〔作者裁決 2026-10-04〕，原驗收單已過期 |
| [claude-compat-endpoint](../handoffs/claude-compat-endpoint.md) | 實作與 cargo/build 已綠；等有真 Claude-compatible base URL＋key 時做使用者實測 |
| [ui-redesign](../handoffs/ui-redesign.md) 觸發條件型對話窗 | 格式轉換更新窗要有含格式轉換的新版；設定外部指定分頁與齒輪紅點要有新版；換幕提醒＋錯誤＋狀態同時要真出錯 |
| [ui-redesign](../handoffs/ui-redesign.md) Windows | 等有 Windows 機：WebView2 連按兩次 Esc 對話窗不被繞過關閉，及分包 1 遺留的 Windows 外觀 |
| [desktop-update-detect](../handoffs/desktop-update-detect.md) 端對端 | 要兩個真 release 才測得到偵測→更新→回退→刪版與跨格式回退；改在第二版發出時驗〔作者裁決 2026-10-12〕 |
| [stable-free-failover](../handoffs/archive/stable-free-failover.md) 真上游換模 | 真上游連續兩次失敗→換模→同句重送，只在假端點驗過；真模型自然遇到時再看（聊天室提示行、重送成功、帳本與 ai-log 對應） |
| [test-harness](../handoffs/archive/test-harness.md) 安裝探測記錄 | 下次實際跑 CLI 安裝／登入流程時，用測試包看 `ai-log` 有 `cli-probe:*`（claude／agy 標 `aiProbe`）與 `cli-setup-terminal:*` 各一筆 |
| [claude-1h-cache](../handoffs/archive/claude-1h-cache.md) 真超額與邊界 | ①Claude 訂閱真的用到超額：跳一次超額提示，文案對到該輪實得時效（本帳號超額未開通）　②玩家 `~/.claude/settings.json` 的 `env` 帶 `FORCE_PROMPT_CACHING_5M=1` 時，`--safe-mode` 下續聊線是否被壓成 5m（帳本 `created_1h_tokens=0`、lanes.json 壽命 300）　③同一條 claude 線閒置超過 1 小時，下一輪改走 rebased　④Windows 上述各項 |
| [test-harness](../handoffs/archive/test-harness.md) 正式包 listener | 要確實隔離資料的環境（獨立 macOS 帳號或 VM）：正式包帶 `TT_HARNESS_ROOT` 啟動不產 harness.json、`lsof` 看不到 listener |
| [long-prompt-scene-hint](../handoffs/archive/long-prompt-scene-hint.md) Windows 真 CLI 讀檔 | 等有 Windows 機且 claude／grok／agy 已登入：中文卡加世界書超過 3 萬字的桌各送一輪——claude 讀 `--system-prompt-file`、grok 讀 `--agent` profile 與 `--prompt-file`、agy 吃 stdin，都回得出話；`cli-prompts/` 呼叫後是空的。原生假 .exe 版已在 Windows CI 過，三家真 CLI 未跑 |
| [multiplayer-room](../handoffs/multiplayer-room.md) 實機 | 要真手機、Windows／Linux 機、牆內網路、真 Workers 帳號與 Caddy 主機：①iOS Safari／Android Chrome 加入選角、送出／收回／跳過、串流、鍵盤彈出，iOS 鍵盤開著點送出是否要兩次（模擬器如此）　②手機 QQ、微信內建瀏覽器出簡中引導、照做能跳外部瀏覽器，QQ 瀏覽器 App 不出　③Windows `cargo check`＋防休眠、Linux `systemd-inhibit`　④牆內系統代理下 WebSocket 走 CONNECT　⑤命令列通道無人值守一晚、撞額度手機能切路線　⑥（已完成：Workers 已部署到 tt-room.pirahus.com）　⑦Node 殼走 Caddy 實跑兩回合。細目見交接檔 |
| [room-status-strip](../handoffs/room-status-strip.md)＋[room-round-timer](../handoffs/room-round-timer.md)＋[room-table-chat](../handoffs/room-table-chat.md) 玩家回饋三案 | 伺服器已部署（2026-11-06）；桌面版打包後作者實機：①桌面版房主的狀態列、toast 與提示音（截圖只有近似版，左欄留白）　②手機房主提示音在 iOS Safari 點過畫面後會不會響　③房主當場開 2 分鐘倒數、玩家橫條、到期自動結算　④聊天：桌面 Enter 送出、中文輸入法選字不誤送、手機熄螢幕回來未讀保留、私訊只有兩人看到 |
| [room-scene-advance-bar](../handoffs/room-scene-advance-bar.md) 換幕鈕在回合列 | 部署／打包後作者實機：手機房主進行中「換幕 ▾」在第二行是否順手、點開「結束本幕」後已送出的行動退回草稿；GM 卡在「這一幕滿了」時回合列主鈕一鍵換幕；桌面版同一列（截圖只有近似版） |
| [room-scene-capacity](../handoffs/room-scene-capacity.md) 幕容量提醒 | 部署＋打包後作者實機：一幕玩到接近容量時房主回合列出現橘色「這一幕快滿了…建議換幕」、玩家看不到、換幕後消失；舊版桌面執行端接新伺服器不出提醒也不報錯 |
| [mvu-zod-semantics](../handoffs/archive/mvu-zod-semantics.md) 網頁 308 標頭 | 網頁版上線後抽查 Cloudflare Pages 上 `/mvu-zod`、`/mvu-zod.html`（308）的實際回應標頭帶隔離 CSP（本機託管伺服器的 308 不帶標頭，驗不到） |
| [vn-mode](../handoffs/archive/vn-mode.md) 包 7b API 立繪來源 | 有額度的 OpenRouter 金鑰：VN 桌立繪來源選 API，生一張定稿（去背後入舞台）＋一張表情差分（裁頭圖當 `input_references` 參考圖），臉層對齊、無接縫。2026-11-01 依官方文件實作，測試金鑰額度 $0 未實測〔作者裁決 2026-11-01〕 |
| [room-player-cards](../handoffs/archive/room-player-cards.md) 真手機 | 部署後用真手機（iOS Safari／Android Chrome）進房：匯入 PNG 卡、建立角色填秘密情報、換圖（相機照片 JPEG／HEIC 轉 PNG）、AI 生圖（房主本機生圖來源 API、有額度的金鑰）按「用這張」；通道只用瀏覽器窗格模擬手機、假圖片端點 |
| [room-phone-gallery](../handoffs/archive/room-phone-gallery.md) 真手機 | iOS Safari、Android Chrome 各一次：編卡「從圖庫選」點縮圖開裁切窗，單指拖動、雙指縮放、滑桿縮放後套用、存檔；上傳相機照片也走一次裁切。通道只用瀏覽器窗格的滑鼠模擬拖動與滑桿 |
| [room-seat-delegate](../handoffs/archive/room-seat-delegate.md) 其他玩家的全員代管 | 作者實機看：全員代管時，非房主玩家手機回合列寫「全員代管中，等房主處理」（實測只截了房主那一版，玩家版只有 UI 測試） |
| [room-open-from-idea](../handoffs/room-open-from-idea.md) 開房窗一句話生成世界 | 2026-11-14 測試通道代測：開房窗三顆鈕與一句話表單預設「朋友自己建角色」過。剩生成後進房與手機入座。 原驗法：打包後作者實機：開房窗三顆鈕；「一句話生成世界開房…」預設「朋友自己建角色」生成後直接進房、桌名是生成標題、開場對一群人說話；朋友手機入座只見「建立角色／匯入角色卡」；切「AI 先生好角色給朋友選」生出的角色可入座；大廳一句話開桌行為不變 |
| [room-roster-avatar](../handoffs/room-roster-avatar.md) 名冊頭像退回角色圖 | 打包後作者實機：桌面版匯入一張有圖的酒館卡開房，朋友手機入座名單（44px）與座位列（22px）看到那張圖的上半部；沒圖的卡是角色名首字加卡色邊框；關掉那張卡的「顯示圖」後重送名冊回到字母圈 |
| [provider-lang-split](../handoffs/provider-lang-split.md) 連線清單依語系分流 | 2026-11-14 測試通道代測：簡中入門卡是矽基流動版、貼 `export …` 即時提示、存檔後卡收起且清單只剩矽基流動＋四個 CLI；切回繁中兩列都在。簡中新裝還沒存金鑰時 API 直連仍列出（已選中的不藏）。剩正式包作者看。 原驗法：打包後作者實機：介面切簡中 → 設定頁連線清單是「矽基流動」＋四個 CLI、沒有 API 直連；切繁中 → 有 API 直連、沒有矽基流動；簡中新裝沒金鑰時首頁入門卡是矽基流動版（申請金鑰鈕開金鑰頁、貼金鑰存檔後卡收起、設定頁顯示矽基流動已選）；簡中貼 `export KEY=…` 這類東西提示指向矽基流動 |
| [vn-story-flags](../handoffs/vn-story-flags.md) VN 遊戲選單與劇本重寫 | 2026-11-14 測試通道代測：選單四組八顆、進度頁、結束本章確認窗、劇本分頁儲存重進仍在、「改成 5 章」重寫成 5 章都過。剩 GM 把旗標寫在頂層時的回饋句。 原驗法：打包後作者實機：VN 桌 Esc 開選單看四組八顆（存讀／進度／美術｜結束本章｜設定／更多｜回標題／回大廳）；「進度」頁章節列「從這章重玩」、關鍵事件、結局；按「結束本章」出確認窗；世界編輯器劇本分頁改一欄按頂列「儲存」後重進仍在；未遊玩桌在劇本分頁文字框輸入「改成 5 章」按「照指示重寫」（燒一次模型）章數變 5；GM 把旗標寫成頂層 `/<id>` 時 AI log 回饋句是「路徑應為 /flags/<id>」 |
| [vn-full-auto-dual-track](../handoffs/vn-full-auto-dual-track.md) VN 開桌一鍵開始 | 2026-11-14 測試通道代測：三步進度列、寫作中關掉再點桌卡回 ②、自動進標題、桌名下主角一句話、舊桌不顯示那行都過。剩進標題時背景是否已在跑。 原驗法：打包後作者實機：大廳「視覺小說」什麼都不填按「直接開始」（燒大綱、建桌、劇本、立繪、背景各一次）看三步進度列「生成大綱→建立世界→撰寫劇本」、自動進標題畫面、桌名下主角一句話、「立繪準備中」與背景已在跑；劇本寫作中按 × 關掉再點桌卡應回 ②；舊桌標題畫面不顯示那行 |
| [vn-audio](../handoffs/vn-audio.md) VN 音訊 | 2026-11-14 測試通道代測：看得到的都過（進桌掛上主題曲、`[bgm]` 換曲新曲從 0 起約 1 秒後舊曲停、靜音音量歸 0 取消回原值）。剩實際聽、Freesound 音效、快轉不連發、匯出再匯入。 原驗法：打包後作者實機：開任一 VN 桌標題畫面應有音樂（主題曲依精靈 ① 的氛圍）；遊玩中 GM 寫 `[bgm: …]` 換曲會 1 秒淡入淡出、`[bgm: off]` 停；遊戲選單「設定」勾「靜音」兩邊都靜、取消回到滑桿值，放開音效滑桿會放一聲試聽（桌裡要有抓過的音效）；重開桌後按 skip 從頭快轉不該連發音效、停下那格的音效照響；設定視窗填 Freesound 金鑰後 `[sfx]` 格到達時放一次（用 Freesound 免費額度，不燒 AI）；匯出 .ttvn 再匯入後標題畫面仍有音樂、沒有音效 |
| [vn-credits](../handoffs/vn-credits.md) VN 感謝名單 | 2026-11-14 測試通道代測：音樂分頁列三首曲名、作者、授權與來源鈕過。剩完結畫面署名與音效段。 原驗法：打包後作者實機：VN 桌標題畫面「美術與音樂」→「音樂」分頁列出主題曲（曲名、Kevin MacLeod、CC BY 4.0）與「來源」鈕（開外部瀏覽器到 incompetech.com）；有抓過音效的桌另出「音效」段；玩到完結畫面「達成 x／y」下方有一到三行署名（音樂一行、音效一行） |
| [vn-pack-opaque](../handoffs/vn-pack-opaque.md) 作品包不透明容器＋音效進包 | 2026-11-14 測試通道代測：匯出檔不是 zip、解壓工具打不開，匯回成功，舊明文包仍可匯入。剩音效進包與 Finder 雙擊。 原驗法：打包後作者實機：有抓過音效的 VN 桌匯出 `.ttvn`，雙擊與解壓軟體都打不開（不認得的檔）；拖回大廳匯入照常，重播桌播到 `[sfx]` 格有音效，「美術與音樂」→「音樂」分頁列音效作者；舊版匯出的明文 `.ttvn` 仍可匯入 |
| [room-small-fixes](../handoffs/room-small-fixes.md) 房間三小事 | 要部署伺服器與真手機：換手機進房，選角頁「這是我的？」；房主名單卡片與故事頁提示；確認後坐回原位且名單少一列；舊裝置回來看到提示；房主用到場整理交回。手機故事看到「某某觀望」；選角簡介沒有 Markdown 原文 |
| [room-player-dice](../handoffs/room-player-dice.md) 房間玩家自己擲骰 | 要部署伺服器與真手機：桌設定開擲骰後開房，手機按骰鈕一鍵擲；全房看到擲果、GM 採用玩家擲果不從骰池另取；擲後關房重開、收回上一回合，擲果都還在 |
| [vn-image-source-flow](../handoffs/vn-image-source-flow.md) VN 開桌先選來源再生成 | 打包後作者實機：額度暫停時按「換來源」「立刻重試」；背景一般失敗的角標與章卡重試；結局畫面沒有角標；英文介面的恢復時間單複數 |
| [vn-stage-layout](../handoffs/vn-stage-layout.md) VN 舞台版面與選項標籤 | 2026-11-14 測試通道代測：單人特寫、選項出現時立繪變暗、倒回看到「上次選的」過（真模型）；雙人同台與退場只看過測試旁白的版面。剩三人同台與 1600×900。 原驗法：打包後作者實機：1600×900 視窗（通道只模擬過 16:9）；雙人與三人同台、特寫與換位；自己玩倒回看到「上次選的」 |
| [ttrpg-builtin-books](../handoffs/ttrpg-builtin-books.md) 內建規則書五本 | 2026-11-14 測試通道過（合 main 後重建的測試包，截圖主線親看）：單人桌與開著房間的桌，五本都在、加入後收成一本、換書只留一套、移除後 0 條；手機尺寸玩家端故事進行中沒有規則書畫面。真模型（claude-sonnet-5-5）：條目進了提示就照該套裁決，但第一句口語行動觸發 0 條、開戰一律擲 d20 排先攻，轉 [ttrpg-book-core-rules](../handoffs/archive/ttrpg-book-core-rules.md) 修好並重測過（2026-11-14 合 main）。剩：真手機與正式包、PF2 差 10 的四級成功度（沒遇到觸發機會） |
| [vn-player-art-style](../handoffs/vn-player-art-style.md) VN 玩家畫風與被拒絕 | 打包後作者實機：VN 桌「美術與音樂」填「這張桌的畫風」（例如像素風）後生一張立繪與一張背景，看有沒有照著畫；開桌精靈第一步點開「這張桌的畫風（選填）」填了再「直接開始」，第一張圖就帶到；故意填會被拒絕的內容，看到「這個 AI 不肯畫…」與三顆按鈕（改畫風／這次不帶我的畫風重試／換立繪來源）都能用、沒有自動重試；改畫風不會自己重生已有的圖 |
