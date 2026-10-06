# 實測佇列（建議順序）

2026-10-02 狀態校正。只收「施工已完成，只剩可執行環境／使用者實機／外部條件驗收」的項目；待實作與待拍板不在此。
各項的驗收細目在各自任務檔與 `handoffs/<id>.md`，此處只排順序與理由。

## 梯 1：本地操作，不花 API 額度

1. [hide-first-action](../handoffs/archive/hide-first-action.md)：桌上角色卡直接「⋯→轉成世界書條目」只跳一次確認即轉成（AI 回應中按轉換顯示忙碌、不轉那半段排梯 2 第 9 項順手看）。

4. [card-mvu-shim](../handoffs/card-mvu-shim.md) 包 1（2026-10-03 已用測試通道代測通過，步驟留作回歸；只讀墊片，test-harness 獨立 root、零 AI 派送）：①匯入 `TestCards/bcd368…png` 開新桌，空桌打開卡片介面：MVU 前端畫出 initvar 的值（用户数据：名称「未知」、资金 100000）　②`post_opening` 寫兩則假 GM 訊息（文字自帶 `<StatusPlaceHolderImpl/>`，因為開場事件不補占位；`<UpdateVariable>` 的 JSONPatch 數字欄用 delta，replace 絕對值會被拒收）：面板顯示第二則的值；收回 → 第一則；復原 → 第二則　③面板開著時寫一則玩家句，再在狀態欄手改资金：面板不重掛、數值自動換　④匯入 `TestCards/DongeonMaster.png`：開場是「开局」畫面、沒被狀態欄搶走；同②③改基础信息.时间　⑤抓兩卡 GM 回合實際送出的提示（只看提示、不送出），看 `<UpdateVariable>` 規定是否還在、有沒有被「不要輸出格式以外的狀態欄」壓掉　⑥ai-log 零派送，quit 後兩個正式目錄 hash 不變。

4a. [card-mvu-shim](../handoffs/card-mvu-shim.md) 包 2a（2026-10-03 已代測通過，步驟留作回歸；卡片寫入，test-harness 獨立 root、零 AI 派送；自製測試卡 `scripts/harness-fixtures/mvu-write-probe.json`）：①匯入測試卡開新桌、貼開場白（first_mes 已自帶占位），打開卡片介面：錢 100（number）、血量 `[5,"生命值"]`（array）、名字＝玩家名、編號 7　②逐一按寫入按鈕（replace／insertOrAssign／insert 不改既有值／delete／updateVariablesWith／setMvuVariable＋replaceMvuData／寫字串 "123"）：值即時變、紀錄區寫「已落檔」；關掉重開面板，值與型別都還在（編號 "123" 仍是 string）、狀態欄（手改面板）同步　③`post_opening` 再寫一則不帶占位的假 GM 訊息，面板換到新樓後按「寫歷史樓」：只有第 0 樓的值變、最新樓不變　④「寫超大值」被拒、面板回到原值　⑤收回上一句：面板值倒回上一樓；復原後再按寫入照常落檔　⑥面板開著時在狀態欄手改「欄位.錢」：面板不重掛、數值自動換，再按卡片寫入不被拒　⑦parseMessage 已於 2026-10-03 代測通過　⑧ai-log 零派送，quit 後兩個正式目錄 hash 不變。

4d. [card-mvu-shim](../handoffs/card-mvu-shim.md) 卡片介面 iframe 內按鈕實際互動（測試通道點不到沙盒 iframe 內的按鈕，留作後續 native 回歸；零 AI 派送）：用 `mvu-write-probe` 開桌、捲到逐字稿中段，面板開著按卡片按鈕寫入（由卡片按鈕送出）：第一筆即落檔、狀態欄同步、捲動位置不動；接著在狀態欄手改再按卡片寫入不被拒。

4e. [refactor-card-png-export](../handoffs/refactor-card-png-export.md)（macOS 部分 2026-10-06 已用測試通道代測通過：三階匯出、陣容欄單一入口匯入 #3、59 MB 大卡往返、QuickLook 縮圖）：剩沒有環境的兩項——①Windows Explorer 看 #3 大卡（數十 MB）的縮圖與檔案大小顯示　②SillyTavern 匯入 #2／#3 PNG 會乾淨拒收（報找不到角色卡，不當成角色卡吃進去）。這兩項驗完本案才算實測完成。

排這梯前先確認該項驗收步驟裡沒有換幕：換幕一定走模型產前情提要摘要（`advance_scene`），避不開。

## 梯 2：要開 API 實聊、會燒額度

| 順位 | 項目 | 為何排這個位置 |
|---|---|---|
| 5 | [refactor-mode-split](../handoffs/refactor-mode-split.md) 剩四洞①②④ GUI 重測、重構中取消；③ 已驗出同桌重跑會清殼，refactor-statusbar-skeleton 已改成已遊玩擋下、未遊玩用原卡清回再跑 | 五卡矩陣、同卡連跑三次、二選一取消、第二段 resume 已於 2026-10-02 測試包跑過。**擋下游最多**：[interface-takeover-spike](../handoffs/interface-takeover-spike.md) 逐型驗卡疊在這條路上 |
| 6 | [ai-card-refactor](../handoffs/ai-card-refactor.md) B 段→A 段 ＋ [person-promote](../handoffs/person-promote.md) ＋ [state-values-mvu](../handoffs/state-values-mvu.md) 真桌 | 三案一鏈，跑一輪同時收。**前置已解除**：`refactor-output-redesign` 已於 2026-08-11 結案，B 段可直接真跑 orc-cave 卡；產物存檔後 A 段走零額度重放，額度只花一次 |
| 8 | [sponsor-features](../handoffs/sponsor-features.md) AI 生圖 | 三個來源各實跑一次＋構圖二選一（選「半身」要出腰以上特寫、2:3 不變、記住上次選擇） |
| 9 | [ui-redesign](../handoffs/ui-redesign.md) 實聊名牌與打字指示 | 自 ui-overhaul 併入：dialogue 事件的名牌版式、串流中打字指示；另順手驗 [hide-first-action](../handoffs/archive/hide-first-action.md) 回應中按「⋯→轉成世界書條目」顯示忙碌、不轉；可搭任一梯 2 項目順手看 |
| 10 | [worldbook-card-import](../handoffs/worldbook-card-import.md) 篇幅與配角解禁 | 用新打的 release 包，同一張世界書卡確認 GM 旁白篇幅放開、配角會開口、角色回覆有內心戲 |
| 11 | [ai-response-stop](../plans/ai-response-stop.md) 順手驗 | 已結案，不專程測。之後實聊（或介面重新設計後整體重測）時，GM 旁白／角色對話各按一次停止：半截有「回應中斷」、下一輪正常 |
| 12 | [ui-redesign](../handoffs/ui-redesign.md) 要 AI 的對話窗 | 重構三窗（進行中、二選一、結果含已取消／部分失敗）、一句話開桌有綱要後底列；可併梯 2 第 5 順手看 |
| 14 | [api-shared-lane](../handoffs/api-shared-lane.md) | 錯認前言者（只有 API 測得到）＋四路快取成對測試（同角色／換角色 × 冷／暖），記絕對 cached tokens；[vendor-prefix-floor](../tasks/vendor-prefix-floor.md) 排在這批數據之後 |
| 15 | [card-arrival-private-leak](../handoffs/card-arrival-private-leak.md) | 多角色桌：回歸事件私設只到 GM |
| 16 | [interface-shell-cleanup](../plans/interface-shell-cleanup.md) | 用 `TestCards/WestFantsy.png` 重構接管跑一輪：面板（地圖 11×7、五分頁）照常渲染、時間跟著回合動；可併第 6 項 ai-card-refactor 五卡矩陣回歸 |
| 17 | [test-harness](../handoffs/archive/test-harness.md) 智慧免費真供應商 | 用 OpenRouter 免費模型桌送一輪：`route` 的智慧免費預覽有值、`ai-log` 的 `api-smart-free` 派送後有同 id 的 `responder` 事件且模型是實際回應者。2026-10-04 代測：route 預覽有值（gemma-4-31b-it:free）過；穩定第一、二名整段被上游限流 429，`responder` 事件未能驗，等穩定第一名可用時重跑一輪 |
| 18 | [card-chat-messages-shim](../handoffs/archive/card-chat-messages-shim.md) 面板開著換值 | NorthHall-structure 桌（沒重構）用低階模型跑一回合：面板開著時 GM 新回覆進來，狀態欄自動換成新回覆的值；可搭任一梯 2 實聊順手看 |
| 19 | [refactor-statusbar-skeleton](../handoffs/archive/refactor-statusbar-skeleton.md) 狀態欄骨架桌 | 已結案、單元測試覆蓋，剩實機觀察，可搭任一梯 2 實聊順手看：①結果框「匯出」（6b）上一輪 AI 重構後沒寫檔、畫面沒報錯，零額度重放正常；下次有授權的 AI 重構時，匯出前後讀 statusMessage 與 `refactor_export_outcome` 回傳定位原因　②Haiku 接管桌偶爾對文字欄下 delta（被規則擋）、`<UpdateVariable>` 的 JSON 字串尾巴多跳脫引號（被容錯跳過），看頻率決定要不要加強提示　③生成中按套用重構／貼開場等排隊時再按停止生成：等待提示照常、截斷回覆落檔後才執行　④真模型一回合佐證排隊套用在 GM 旁白落檔後才執行（這筆回覆留在套用前的桌況）　⑤待查證：酒館 regex 替換字串與訊息顯示會不會替換 `{{user}}`／`{{char}}`，會的話面板也應替換（只查規格） |
| 20 | [interface-scene-change](../handoffs/interface-scene-change.md) 介面換幕 native 核對 | 西幻接管桌（測試通道代測已過資料面與 srcdoc 渲染）：真視窗裡打開卡片介面，按介面工具列「換幕」→ 介面留著、正文槽是前情提要、右側時間地點與地圖照舊；在介面內點區域／推薦行動送出一回合，面板跟動。測試通道拍不到也點不到 iframe 內容，只能真人看 |
| 21 | [image-model-picker](../handoffs/image-model-picker.md) 生圖統一 PNG 真打 | 單元測試只用 mock：①付費生圖模型回 JPEG／WebP／遠端 URL 時真打一次，圖進圖庫且是 PNG（看圖庫檔頭）　②codex／agy／grok 真生圖一次，走「讀進記憶體→清工作目錄→轉 PNG→進圖庫」新流程；可併第 8 項一起跑 |
| 22 | [image-model-free-tier-hide](../handoffs/image-model-free-tier-hide.md) 免費層藏生圖模型 | 測試通道只用本機假 `/key`：①真 OpenRouter 免費 key 與付費 key 各開一次設定頁「AI 連線」，免費隱藏生圖模型、付費顯示　②整個關掉設定視窗再開，快取首屏不閃；可併第 21 項一起跑 |

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
| [desktop-update-detect](../handoffs/desktop-update-detect.md) 端對端 | 要兩個真 release 才測得到偵測→更新→回退→刪版與跨格式回退；第一個帶更新功能的正式版發出前必須驗過 |
| [stable-free-failover](../handoffs/archive/stable-free-failover.md) 真上游換模 | 真上游連續兩次失敗→換模→同句重送，只在假端點驗過；真模型自然遇到時再看（聊天室提示行、重送成功、帳本與 ai-log 對應） |
| [test-harness](../handoffs/archive/test-harness.md) 安裝探測記錄 | 下次實際跑 CLI 安裝／登入流程時，用測試包看 `ai-log` 有 `cli-probe:*`（claude／agy 標 `aiProbe`）與 `cli-setup-terminal:*` 各一筆 |
| [claude-1h-cache](../handoffs/archive/claude-1h-cache.md) 真超額與邊界 | ①Claude 訂閱真的用到超額：跳一次超額提示，文案對到該輪實得時效（本帳號超額未開通）　②玩家 `~/.claude/settings.json` 的 `env` 帶 `FORCE_PROMPT_CACHING_5M=1` 時，`--safe-mode` 下續聊線是否被壓成 5m（帳本 `created_1h_tokens=0`、lanes.json 壽命 300）　③同一條 claude 線閒置超過 1 小時，下一輪改走 rebased　④Windows 上述各項 |
| [test-harness](../handoffs/archive/test-harness.md) 正式包 listener | 要確實隔離資料的環境（獨立 macOS 帳號或 VM）：正式包帶 `TT_HARNESS_ROOT` 啟動不產 harness.json、`lsof` 看不到 listener |
| [long-prompt-scene-hint](../handoffs/archive/long-prompt-scene-hint.md) Windows 真 CLI 讀檔 | 等有 Windows 機且 claude／grok／agy 已登入：中文卡加世界書超過 3 萬字的桌各送一輪——claude 讀 `--system-prompt-file`、grok 讀 `--agent` profile 與 `--prompt-file`、agy 吃 stdin，都回得出話；`cli-prompts/` 呼叫後是空的。原生假 .exe 版已在 Windows CI 過，三家真 CLI 未跑 |
