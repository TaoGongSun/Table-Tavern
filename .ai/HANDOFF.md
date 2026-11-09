# 工作線索引

一線一檔於 [handoffs/](handoffs/)，交接＝就地改寫該檔。開工先整份讀該檔。

分類規則：仍需程式施工／規格落地才放「進行中」；施工已完成、只剩可執行環境／使用者實機／外部條件驗收則移到「等實機驗收」，並同步列入[實測佇列](reference/verification-queue.md)。

## 進行中
- [release-8](handoffs/release-8.md) — v0.5.3 文件、版號、房間伺服器部署完成並已合 main；作者親口說才推 tag
- [vn-stage-layout](handoffs/vn-stage-layout.md) — VN 立繪放大、對話框與選項標籤、臉框跟著立繪來源（已合 main，等打包後實機）
- [vn-image-source-flow](handoffs/vn-image-source-flow.md) — VN 開桌先選來源再生成、agy 額度用完暫停續生、失敗講清楚（已合 main，等打包後實機）
- [room-player-dice](handoffs/room-player-dice.md) — 房間玩家自己擲骰：已進 main，待部署後實機跑一回合
- [multiplayer-friends-test](handoffs/multiplayer-friends-test.md) — 給朋友測試的多人連線版：房間預設正式伺服器、簡中下載頁以多人連線為主軸（已上線，等朋友實機測試）
- [card-mvu-shim](handoffs/card-mvu-shim.md) — 卡片介面沙盒墊 MVU 讀寫變數：全部已合併 main，剩實測佇列梯 1 項目（含 iframe 內按鈕實際互動）

## 等實機驗收（順序見[實測佇列](reference/verification-queue.md)）
- [vn-sprite-exit](handoffs/vn-sprite-exit.md) — VN 角色同地點離開時立繪退場：已合 main，待部署，剩真模型遵循率未驗。
- [room-small-fixes](handoffs/room-small-fixes.md) — 房間三小事：選角簡介剝 Markdown、手機顯示某某觀望、換裝置由玩家認領＋房主確認接回座位：已合 main（2026-11-08）、Sol 方案審 2 輪＋驗收 2 輪、Fable 常識審 3 次、截圖主線親看；要先部署伺服器，等作者實機（佇列梯 3）
- [ttrpg-combat-tracker](handoffs/ttrpg-combat-tracker.md) — 跑團戰鬥層：先攻、HP／狀態效果由 app 加減、傷害綁骰池、拒收白話標記可點開手改：已合 main（2026-11-08）、Sol 方案審 2 輪＋驗收包 1／包 2 各 2 輪、Fable 常識審 3 次、截圖主線親看；等作者實機（佇列梯 2 第 34 項）
- [ttrpg-rules-system](handoffs/ttrpg-rules-system.md) — 跑團 v1：GM 骰池＋單人骰鈕＋內建 5e SRD 5.1（2026-11-08 合 main，等作者實機，佇列梯 2 第 33 項）
- [gm-relay-unify](handoffs/gm-relay-unify.md) — GM 接力單人／房間合成後端一份驅動：已合 main（2026-11-08）、Sol 方案審 2 輪＋驗收包 1／包 2 各 2 輪、Fable 常識審、截圖主線親看；等作者實機
- [table-status-panel](handoffs/table-status-panel.md) — 通用狀態欄：無卡格式桌由 app 種角色格＋追蹤三欄、模型只報變動、硬預算、桌面側欄／手機抽屜、房間廣播：已合 main（2026-11-07）、Sol 方案審 2 輪＋驗收 2 輪、opus-review 驗收包 2／3、截圖主線親看；等作者實機
- [lobby-table-badges](handoffs/lobby-table-badges.md) — 大廳不分單人／多人：房間釘頂＋色帶、8 張起篩選籤、入口卡重排：已合 main（2026-11-07）、opus-review 驗收 1 輪 PASS、截圖主線親看；等作者實機看
- [vn-pack-opaque](handoffs/vn-pack-opaque.md) — 作品包改不透明容器（一般人打不開的 .ttvn）＋音效隨包帶走並進名單：已合 main（2026-11-07）、Sol 方案審＋驗收各 1 輪無必改；等作者試玩
- [vn-credits](handoffs/vn-credits.md) — VN 感謝名單：完結字卡精簡署名＋「美術與音樂」面板音樂頁（曲名、作者、授權、來源連結）：已合 main（2026-11-07）、Sol 驗收 1 輪 DONE；等作者試玩
- [vn-audio](handoffs/vn-audio.md) — VN 音訊：零金鑰 BGM（Incompetech、GM `[bgm: 情緒]` 標記自動切換、標題主題曲）＋選填 Freesound 音效、音量設定、作品包帶 BGM：已合 main（2026-11-07）、Sol 方案審 2 輪＋驗收 2 輪 PASS；等作者試玩
- [vn-full-auto-dual-track](handoffs/vn-full-auto-dual-track.md) — VN 開桌一鍵開始（① 「直接開始」主鈕串起大綱→建桌→劇本→標題畫面、首幕背景並行、劇本失敗停在 ②、標題畫面露主角一句話）：已合 main（2026-11-07）、Sol 1 輪 PASS；等作者試玩
- [vn-story-flags](handoffs/vn-story-flags.md) — 劇情旗標剩餘缺口五項（回饋句指路、頂列一鍵存劇本、照指示重寫劇本、VN 遊戲選單重排成四組八顆＋「進度」頁）：已合 main（2026-11-07）、Sol 4 輪 PASS；等作者試玩
- [provider-lang-split](handoffs/provider-lang-split.md) — 桌面版 AI 連線清單依語系分流（簡中矽基流動取代 OpenRouter、入門卡簡中推矽基流動）：已合併 main（2026-11-07），已隨 v0.5.2 發布（2026-11-07）
- [room-roster-avatar](handoffs/room-roster-avatar.md) — 名冊頭像沒有 `avatar.png` 時退回角色圖、入座名單沒圖畫字母圈（玩家回饋「網頁端沒有顯示頭像」）：已合併 main（2026-11-07），已隨 v0.5.2 發布（2026-11-07）
- [room-open-from-idea](handoffs/room-open-from-idea.md) — 開房窗接「一句話生成世界」、預設朋友自己建角色（房主回饋「世界觀隨機」＝不選卡開空桌）：已合併 main（2026-11-07），已隨 v0.5.2 發布（2026-11-07）
- [room-scene-advance-bar](handoffs/room-scene-advance-bar.md) — 換幕鈕搬到回合列（房主回饋「需要換幕」，功能已有但藏在房主頁）：已合併 main（2026-11-06），已隨 v0.5.2 發布（2026-11-07）
- [room-scene-capacity](handoffs/room-scene-capacity.md) — 幕容量提醒（執行端報 `capacityHint`、房主回合列先提醒再換幕）：已合併 main（2026-11-06），已隨 v0.5.2 發布（2026-11-07）
- [room-status-strip](handoffs/room-status-strip.md) — 桌邊狀態列＋房主提醒（玩家回饋第 1 案）：已合併 main（2026-11-06），等作者實機看桌面版與手機
- [room-round-timer](handoffs/room-round-timer.md) — 回合倒數由房主當場開關（玩家回饋第 2 案）：已合併 main（2026-11-06），伺服器已部署（2026-11-06），已隨 v0.5.2 發布（2026-11-07）
- [room-table-chat](handoffs/room-table-chat.md) — 桌邊聊天（玩家回饋第 3 案）：已合併 main（2026-11-06），伺服器已部署（2026-11-06），已隨 v0.5.2 發布（2026-11-07）
- [vn-setup-wizard](handoffs/vn-setup-wizard.md) — VN 開桌精靈三步（沒想法也能開桌、劇本規格層、美術準備永遠可開始）、暫定 `[bg]` 提早排隊與後端自動定稿、換時段色調遮罩、美術一覽：三包已合 main（2026-11-06）、各包 Sol 2 輪＋常識審；等作者試玩（實測佇列第 30 項）
- [vn-ui-polish](handoffs/vn-ui-polish.md) — VN 播放器以玩家直覺調整（點畫面／鍵盤推進、背景鋪滿、快捷列在對話框下、遊戲選單、標題與立繪修正）：已合 main、Sol 驗收 PASS；等作者試玩（實測佇列第 30 項）
- [multiplayer-room](handoffs/multiplayer-room.md) — 多人跑團房間 v1（桌面版當 GM 執行端、玩家用手機加入、房主可在電腦上直接玩）：包 0–7 全部通過驗收、2026-11-03 已進 main；剩作者真手機（iOS／Android／QQ／微信）、Windows 防休眠、牆內、真 Workers 與 Caddy 部署等實機項目（實測佇列梯 3）
- [desktop-csp](handoffs/desktop-csp.md) — 桌面版 CSP：已進 main、macOS 測試通道過；剩 Windows 安全驗收（實測佇列梯 1 第 4i）
- [desktop-tour](handoffs/desktop-tour.md) — 網頁版的桌面版導覽頁：2026-10-23 已上線（main 7f686cb）、WebKit e2e 與站上實開都過，無剩餘事項
- [release-2-ci-windows](handoffs/release-2-ci-windows.md) — 桌面版首個正式版 v0.3.0 已發布（2026-10-12，未簽章）；剩作者 Windows／Mac 照繞過步驟下載安裝能開
- [image-model-free-tier-hide](handoffs/image-model-free-tier-hide.md) — OpenRouter 免費層不顯示生圖模型選單：已進 main，免費 key 與整窗重開實測過，只剩付費 key 半（列缺數據，實測佇列梯 2 第 22）
- [image-model-picker](handoffs/image-model-picker.md) — 生圖來源不列 claude、生圖模型下拉、免費 key 專屬錯誤、生圖統一存 PNG：已進 main，剩真模型與 CLI 真生圖（實測佇列梯 2 第 21）
- [card-arrival-private-leak](handoffs/card-arrival-private-leak.md) — 回歸事件漏私設：已進 main，等實機看私設只到 GM
- [ui-redesign](handoffs/ui-redesign.md) — 介面整體重新設計：五包已進 main、範例桌詢問已過；剩重構三窗、實聊名牌與打字指示（自 ui-overhaul 併入）、格式轉換更新等觸發條件型對話窗、Windows 等未實機驗項目
- [desktop-update-detect](handoffs/desktop-update-detect.md) — 桌面版 App 內更新與回退：包 1–5 完成、GUI 煙霧測試過；剩兩個真 release 的端對端驗收，改在第二版發出時驗
- [worldbook-card-import](handoffs/worldbook-card-import.md) — 世界書卡匯入：本地操作 2026-10-02 實機過，剩篇幅／配角解禁實聊排梯 2 第 10
- [sponsor-features](handoffs/sponsor-features.md) — 贊助三項：贊助狀態與作者頁 2026-10-02 實機過，剩 AI 生圖排梯 2 第 8
- [refactor-survey-spans](handoffs/refactor-survey-spans.md) — 盤點四分類＋照搬零輸出：T4 三項過，API 退 GM 檔那項延到真用 API 模式時驗
- [refactor-dispatch](handoffs/refactor-dispatch.md) — AI 重構提速省費：P4–P6 已隨 refactor-survey-spans T4① 過，只剩 P8（API 模式跑重構）
- [prompt-cache-optimization](handoffs/prompt-cache-optimization.md) — 提示詞快取優化：程式面包 1–7 完成、額度分頁全驗過；剩離開提醒自然遇到再看、undo 截尾優化等非必要項，OpenRouter／API 已收束出本案範圍
- [i18n-more-languages](handoffs/i18n-more-languages.md) — 十國語言：機械關卡持續綠，人眼審校等網頁版與卡片匯出（refactor-card-png-export）都做完後一起驗〔作者裁決 2026-10-04〕
- [api-shared-lane](handoffs/api-shared-lane.md) — API 路徑改走 chars 共線：包 A／B 完成、Sol 過，剩錯認前言者＋api／codex 快取成對測試（agy、grok 已改走 lane 續聊）
- [claude-compat-endpoint](handoffs/claude-compat-endpoint.md) — Claude 相容端點：實作完成、cargo/build 雙驗證綠，等使用者用真相容端點實測後結案
- [refactor-card-png-export](handoffs/refactor-card-png-export.md) — 重構卡 PNG 匯出：包 A–D 已進 main、macOS 測試通道實測過；剩 Windows Explorer 縮圖與 SillyTavern 拒收兩項（實測佇列梯 1 第 4e）
