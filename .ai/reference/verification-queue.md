# 實測佇列（建議順序）

2026-10-02 狀態校正。只收「施工已完成，只剩可執行環境／使用者實機／外部條件驗收」的項目；待實作與待拍板不在此。
各項的驗收細目在各自任務檔與 `handoffs/<id>.md`，此處只排順序與理由。

## 梯 1：本地操作，不花 API 額度

一次開 app 就能掃完，測完可直接結一案。

| 順位 | 項目 | 驗什麼 |
|---|---|---|
| 1 | [state-values-mvu](../handoffs/state-values-mvu.md) 兩處面板 | 分支指認下拉（包 5）與機制帳本（包 8）——`[initvar]` 條目在匯入時就建好狀態樹、辨識失敗當場記帳，所以匯入卡片即可看，不必跑模型 |
| 2 | [state-values-mvu](../handoffs/state-values-mvu.md) 跳動記號（包 6） | 捏資料強制驗一次（2026-08-17 拍板），步驟見下節。真跑撞門檻的機率太低，不等真桌 |
| 3 | [worldbook-card-import](../handoffs/worldbook-card-import.md) 匯入與條目編輯 | 匯 PNG 世界書看 17 條入列、條目就地展開、換編輯對象自動存、空桌回收不再誤刪整桌——全是本地操作 |
| 4 | [sponsor-features](../handoffs/sponsor-features.md) 贊助狀態與作者頁 | `.ttpack` 丟進「文件/TableTavern」解鎖、刪檔還原；作者頁與 +5 配色一起看過 |
| 5 | [dialog-button-labels](../handoffs/dialog-button-labels.md) 對話窗按鈕 | ru 未儲存離開已過；切 zh-TW／en／de 點開刪角色、未儲存離開、轉成世界書條目、匯入完成通知，按鈕是介面語言、長譯文不截 |
| 6 | [ui-redesign](../handoffs/ui-redesign.md) 不需 AI 的對話窗 | 範例桌詢問（設定改語言）、格式轉換更新窗（需有含格式轉換的新版）、設定外部指定分頁與齒輪紅點；分包 1 遺留的換幕提醒＋錯誤＋狀態同時、⋯ 鍵盤 |
| 7 | [api-cache-visibility](../handoffs/api-cache-visibility.md) ＋ [prompt-cache-optimization](../handoffs/prompt-cache-optimization.md) 額度分頁 | 帳本已有 137 輪 api（含命中與 0）、grok 37 輪、agy 16 輪 reported 的真資料；開額度分頁看 api 列顯示真實命中率（0 輪顯示 0.0% 而非「—」）、grok 金額與 agy 列 |
| 8 | [stream-failure-visible](../plans/stream-failure-visible.md) T6 | 拿留有舊空白事件的桌連按收回再復原，復原放回有內容那則 |

排這梯前先確認該項驗收步驟裡沒有換幕：換幕一定走模型產前情提要摘要（`advance_scene`），避不開。

### 跳動記號怎麼捏

`detect_jumps` 只在模型回報新值時比對，且門檻是「絕對幅度 ≥30 **且** 佔較大值四成」，正常玩不保證撞得到。前端只認 `jumps` 這張表有沒有該路徑（[StateBar.tsx:114](../../src/features/table-state/StateBar.tsx)），直接寫進去就會畫記號：

1. **關掉 app**（開著改會被記憶體狀態覆寫）。
2. 挑一張可丟的測試桌——**別動正在玩的桌**，點記號會真的把該欄釘成計數器、寫進機制設定。
3. 編輯 `~/Documents/TableTavern/worlds/<桌 ULID>/state.json`，在 `"state": { … }` 物件內加一筆：
   ```json
   "jumps": { "CharSheet.Basic": "+45" }
   ```
   key ＝ 該檔 `state.tree` 裡的葉節點路徑（點分），挑一個數值型欄位最像真實情況；value ＝ 要顯示的標記字串。`jumps` 是 `#[serde(default)]`，平常空的不落檔，手加即讀得到。
4. 開 app 進那桌 → 狀態欄該欄數值旁出現 `⚠ +45` 按鈕。
5. 點下去 → 該欄被釘成計數器，記號消失、之後那欄不再標。
6. 驗完刪掉那張測試桌。

驗到的是前端呈現與點擊行為；偵測門檻本身 cargo test 已蓋。

## 梯 2：要開 API 實聊、會燒額度

| 順位 | 項目 | 為何排這個位置 |
|---|---|---|
| 5 | [refactor-mode-split](../handoffs/refactor-mode-split.md) 五卡矩陣＋同卡連跑三次 | **擋下游最多**：[refactor-card-png-export](../tasks/refactor-card-png-export.md) 待開工首包（套用映射持久化）與 [interface-takeover-spike](../handoffs/interface-takeover-spike.md) 逐型驗卡都疊在這條路上。程式碼 2026-08-14 才寫完，出問題時記憶最新、最好修 |
| 6 | [ai-card-refactor](../handoffs/ai-card-refactor.md) B 段→A 段 ＋ [person-promote](../handoffs/person-promote.md) ＋ [state-values-mvu](../handoffs/state-values-mvu.md) 真桌 | 三案一鏈，跑一輪同時收。**前置已解除**：`refactor-output-redesign` 已於 2026-08-11 結案，B 段可直接真跑 orc-cave 卡；產物存檔後 A 段走零額度重放，額度只花一次 |
| 7 | [ai-table-generator](../handoffs/ai-table-generator.md) 一句話開桌 | 六項一輪跑完：開視窗→生成大綱→重骰→改大綱→AI 生成角色→照大綱開桌；順手驗單人設定不錨定角色數、換語言後生成跟著換 |
| 8 | [sponsor-features](../handoffs/sponsor-features.md) AI 生圖 | 三個來源各實跑一次＋構圖二選一（選「半身」要出腰以上特寫、2:3 不變、記住上次選擇） |
| 9 | [ui-redesign](../handoffs/ui-redesign.md) 實聊名牌與打字指示 | 自 ui-overhaul 併入：dialogue 事件的名牌版式、串流中打字指示；可搭任一梯 2 項目順手看 |
| 10 | [worldbook-card-import](../handoffs/worldbook-card-import.md) 篇幅與配角解禁 | 用新打的 release 包，同一張世界書卡確認 GM 旁白篇幅放開、配角會開口、角色回覆有內心戲 |
| 11 | [ai-response-stop](../plans/ai-response-stop.md) 順手驗 | 已結案，不專程測。之後實聊（或介面重新設計後整體重測）時，GM 旁白／角色對話各按一次停止：半截有「回應中斷」、下一輪正常 |
| 12 | [ui-redesign](../handoffs/ui-redesign.md) 要 AI 的對話窗 | 重構三窗（進行中、二選一、結果含已取消／部分失敗）、一句話開桌有綱要後底列；可併梯 2 第 5、7 順手看 |
| 13 | [free-player-onboarding](../plans/free-player-onboarding.md) | 2026-09-18 已進 main、未實機驗：計畫檔第 1／2／3 階段驗收各節（空白設定走 OpenRouter 一鍵連接、穩定免費選模、推薦與限免提示） |
| 14 | [api-shared-lane](../handoffs/api-shared-lane.md) | 錯認前言者（只有 API 測得到）＋四路快取成對測試（同角色／換角色 × 冷／暖），記絕對 cached tokens；[vendor-prefix-floor](../tasks/vendor-prefix-floor.md) 排在這批數據之後 |

## 梯 3：等外部條件，不排時程

機會來了順手做，不佔排程。

| 項目 | 卡在哪 |
|---|---|
| [refactor-survey-spans](../handoffs/refactor-survey-spans.md) T4 ② ＋ [refactor-dispatch](../handoffs/refactor-dispatch.md) P8 | 要真的用 API 模式跑一次才看得到 jsonl lane；CLI 模式測不到 |
| [stream-failure-visible](../plans/stream-failure-visible.md) T3–T5、T7 | 失敗態碰運氣重現，遇到再照計畫檔逐項核對 |
| [i18n-more-languages](../handoffs/i18n-more-languages.md) | 2026-08-17 拍板延到全 app 功能定案後一次驗，原驗收單已過期 |
| [claude-compat-endpoint](../handoffs/claude-compat-endpoint.md) | 實作與 cargo/build 已綠；等有真 Claude-compatible base URL＋key 時做使用者實測 |
| [ui-redesign](../handoffs/ui-redesign.md) Windows | 等有 Windows 機：WebView2 連按兩次 Esc 對話窗不被繞過關閉，及分包 1 遺留的 Windows 外觀 |
| [desktop-update-detect](../handoffs/desktop-update-detect.md) 端對端 | 要兩個真 release 才測得到偵測→更新→回退→刪版與跨格式回退；第一個帶更新功能的正式版發出前必須驗過 |
