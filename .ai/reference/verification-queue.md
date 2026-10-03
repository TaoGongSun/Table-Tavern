# 實測佇列（建議順序）

2026-10-02 狀態校正。只收「施工已完成，只剩可執行環境／使用者實機／外部條件驗收」的項目；待實作與待拍板不在此。
各項的驗收細目在各自任務檔與 `handoffs/<id>.md`，此處只排順序與理由。

## 梯 1：本地操作，不花 API 額度

1. [hide-first-action](../handoffs/archive/hide-first-action.md)：桌上角色卡直接「⋯→轉成世界書條目」只跳一次確認即轉成（AI 回應中按轉換顯示忙碌、不轉那半段排梯 2 第 9 項順手看）。
2. [menu-keyboard-webkit](../handoffs/menu-keyboard-webkit.md)：世界設定頁先點文字框、再滑鼠開世界書 ⋯：第一項有底色；↑↓ 循環每步都看得到外框；滑鼠移入換亮項、再按方向鍵從該項接續；Esc 回 ⋯ 鈕有外框；開 VoiceOver 混用滑鼠與方向鍵，播報不亂跳、停用項讀得到但按了不執行。

3. [quota-insufficient-alert](../handoffs/quota-insufficient-alert.md)：設定填一把無效 API 金鑰觸發失敗（不花額度）。①打字送出 → 攔截式彈窗只有「關閉」、開窗焦點在「關閉」、Esc 可關、點遮罩不關，輸入框回到原文、逐字稿不留那句　②卡片介面覆蓋層開著時從卡片送出 → 彈窗疊在覆蓋層上面　③讓收回不成立（例如 GM 已寫入登場事件，或直接看彈窗帶原文的情況）→ 彈窗裡的唯讀原文可選取、能複製出來。

排這梯前先確認該項驗收步驟裡沒有換幕：換幕一定走模型產前情提要摘要（`advance_scene`），避不開。

## 梯 2：要開 API 實聊、會燒額度

| 順位 | 項目 | 為何排這個位置 |
|---|---|---|
| 5 | [refactor-mode-split](../handoffs/refactor-mode-split.md) 剩四洞①②④ GUI 重測、重構中取消；③ 已驗出同桌重跑會清殼，refactor-statusbar-skeleton 已改成已遊玩擋下、未遊玩用原卡清回再跑 | 五卡矩陣、同卡連跑三次、二選一取消、第二段 resume 已於 2026-10-02 測試包跑過。**擋下游最多**：[refactor-card-png-export](../tasks/refactor-card-png-export.md) 待開工首包（套用映射持久化）與 [interface-takeover-spike](../handoffs/interface-takeover-spike.md) 逐型驗卡都疊在這條路上 |
| 6 | [ai-card-refactor](../handoffs/ai-card-refactor.md) B 段→A 段 ＋ [person-promote](../handoffs/person-promote.md) ＋ [state-values-mvu](../handoffs/state-values-mvu.md) 真桌 | 三案一鏈，跑一輪同時收。**前置已解除**：`refactor-output-redesign` 已於 2026-08-11 結案，B 段可直接真跑 orc-cave 卡；產物存檔後 A 段走零額度重放，額度只花一次 |
| 7 | [ai-table-generator](../handoffs/ai-table-generator.md) 一句話開桌 | 六項一輪跑完：開視窗→生成大綱→重骰→改大綱→AI 生成角色→照大綱開桌；順手驗單人設定不錨定角色數、換語言後生成跟著換 |
| 8 | [sponsor-features](../handoffs/sponsor-features.md) AI 生圖 | 三個來源各實跑一次＋構圖二選一（選「半身」要出腰以上特寫、2:3 不變、記住上次選擇） |
| 9 | [ui-redesign](../handoffs/ui-redesign.md) 實聊名牌與打字指示 | 自 ui-overhaul 併入：dialogue 事件的名牌版式、串流中打字指示；另順手驗 [hide-first-action](../handoffs/archive/hide-first-action.md) 回應中按「⋯→轉成世界書條目」顯示忙碌、不轉；可搭任一梯 2 項目順手看 |
| 10 | [worldbook-card-import](../handoffs/worldbook-card-import.md) 篇幅與配角解禁 | 用新打的 release 包，同一張世界書卡確認 GM 旁白篇幅放開、配角會開口、角色回覆有內心戲 |
| 11 | [ai-response-stop](../plans/ai-response-stop.md) 順手驗 | 已結案，不專程測。之後實聊（或介面重新設計後整體重測）時，GM 旁白／角色對話各按一次停止：半截有「回應中斷」、下一輪正常 |
| 12 | [ui-redesign](../handoffs/ui-redesign.md) 要 AI 的對話窗 | 重構三窗（進行中、二選一、結果含已取消／部分失敗）、一句話開桌有綱要後底列；可併梯 2 第 5、7 順手看 |
| 13 | [free-player-onboarding](../plans/free-player-onboarding.md) | 2026-09-18 已進 main、未實機驗：計畫檔第 1／2／3 階段驗收各節（空白設定走 OpenRouter 一鍵連接、穩定免費選模、推薦與限免提示） |
| 14 | [api-shared-lane](../handoffs/api-shared-lane.md) | 錯認前言者（只有 API 測得到）＋四路快取成對測試（同角色／換角色 × 冷／暖），記絕對 cached tokens；[vendor-prefix-floor](../tasks/vendor-prefix-floor.md) 排在這批數據之後 |
| 15 | [card-arrival-private-leak](../handoffs/card-arrival-private-leak.md) ＋ [grok-cache-miss](../handoffs/grok-cache-miss.md) 角色線 | 多角色桌：回歸事件私設只到 GM；grok 通道讓角色連接三輪以上，`chars:grok-4.6:<角色 id>` 的 cached_tokens 隨對話增長，換角色／改卡／換幕後不每輪重開 |
| 16 | [interface-shell-cleanup](../plans/interface-shell-cleanup.md) | 用 `TestCards/WestFantsy.png` 重構接管跑一輪：面板（地圖 11×7、五分頁）照常渲染、時間跟著回合動；可併第 6 項 ai-card-refactor 五卡矩陣回歸 |
| 17 | [test-harness](../handoffs/archive/test-harness.md) 智慧免費真供應商 | 用 OpenRouter 免費模型桌送一輪：`route` 的智慧免費預覽有值、`ai-log` 的 `api-smart-free` 派送後有同 id 的 `responder` 事件且模型是實際回應者；可搭第 13 項 free-player-onboarding 順手看 |
| 18 | [card-chat-messages-shim](../handoffs/archive/card-chat-messages-shim.md) 面板開著換值 | NorthHall-structure 桌（沒重構）用低階模型跑一回合：面板開著時 GM 新回覆進來，狀態欄自動換成新回覆的值；可搭任一梯 2 實聊順手看 |
| 19 | [refactor-statusbar-skeleton](../handoffs/archive/refactor-statusbar-skeleton.md) 狀態欄骨架桌 | 已結案、單元測試覆蓋，剩實機觀察，可搭任一梯 2 實聊順手看：①結果框「匯出」（6b）上一輪 AI 重構後沒寫檔、畫面沒報錯，零額度重放正常；下次有授權的 AI 重構時，匯出前後讀 statusMessage 與 `refactor_export_outcome` 回傳定位原因　②Haiku 接管桌偶爾對文字欄下 delta（被規則擋）、`<UpdateVariable>` 的 JSON 字串尾巴多跳脫引號（被容錯跳過），看頻率決定要不要加強提示　③生成中按套用重構／貼開場等排隊時再按停止生成：等待提示照常、截斷回覆落檔後才執行　④真模型一回合佐證排隊套用在 GM 旁白落檔後才執行（這筆回覆留在套用前的桌況）　⑤待查證：酒館 regex 替換字串與訊息顯示會不會替換 `{{user}}`／`{{char}}`，會的話面板也應替換（只查規格） |

## 梯 3：等外部條件，不排時程

機會來了順手做，不佔排程。

| 項目 | 卡在哪 |
|---|---|
| [refactor-survey-spans](../handoffs/refactor-survey-spans.md) T4 ② ＋ [refactor-dispatch](../handoffs/refactor-dispatch.md) P8 | 要真的用 API 模式跑一次才看得到 jsonl lane；CLI 模式測不到 |
| [stream-failure-visible](../plans/stream-failure-visible.md) T3–T5、T7 | 失敗態碰運氣重現，遇到再照計畫檔逐項核對 |
| [i18n-more-languages](../handoffs/i18n-more-languages.md) | 2026-08-17 拍板延到全 app 功能定案後一次驗，原驗收單已過期 |
| [claude-compat-endpoint](../handoffs/claude-compat-endpoint.md) | 實作與 cargo/build 已綠；等有真 Claude-compatible base URL＋key 時做使用者實測 |
| [ui-redesign](../handoffs/ui-redesign.md) 觸發條件型對話窗 | 格式轉換更新窗要有含格式轉換的新版；設定外部指定分頁與齒輪紅點要有新版；換幕提醒＋錯誤＋狀態同時要真出錯 |
| [ui-redesign](../handoffs/ui-redesign.md) Windows | 等有 Windows 機：WebView2 連按兩次 Esc 對話窗不被繞過關閉，及分包 1 遺留的 Windows 外觀 |
| [desktop-update-detect](../handoffs/desktop-update-detect.md) 端對端 | 要兩個真 release 才測得到偵測→更新→回退→刪版與跨格式回退；第一個帶更新功能的正式版發出前必須驗過 |
| [test-harness](../handoffs/archive/test-harness.md) 安裝探測記錄 | 下次實際跑 CLI 安裝／登入流程時，用測試包看 `ai-log` 有 `cli-probe:*`（claude／agy 標 `aiProbe`）與 `cli-setup-terminal:*` 各一筆 |
| [test-harness](../handoffs/archive/test-harness.md) 正式包 listener | 要確實隔離資料的環境（獨立 macOS 帳號或 VM）：正式包帶 `TT_HARNESS_ROOT` 啟動不產 harness.json、`lsof` 看不到 listener |
