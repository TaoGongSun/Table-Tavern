# 實測佇列（建議順序）

2026-08-18 重編。只收「實作已完成、等使用者實機驗收」的項目；待實作與待拍板不在此。
各項的驗收細目在各自任務檔與 `handoffs/<id>.md`，此處只排順序與理由。

## 梯 1：本地操作，不花 API 額度

一次開 app 就能掃完，測完可直接結一案。

| 順位 | 項目 | 驗什麼 |
|---|---|---|
| 1 | [state-values-mvu](../handoffs/state-values-mvu.md) 兩處面板 | 分支指認下拉（包 5）與機制帳本（包 8）——`[initvar]` 條目在匯入時就建好狀態樹、辨識失敗當場記帳，所以匯入卡片即可看，不必跑模型 |
| 2 | [state-values-mvu](../handoffs/state-values-mvu.md) 跳動記號（包 6） | 捏資料強制驗一次（2026-08-17 拍板），步驟見下節。真跑撞門檻的機率太低，不等真桌 |
| 3 | [worldbook-card-import](../handoffs/worldbook-card-import.md) 匯入與條目編輯 | 匯 PNG 世界書看 17 條入列、條目就地展開、換編輯對象自動存、空桌回收不再誤刪整桌——全是本地操作 |
| 4 | [sponsor-features](../handoffs/sponsor-features.md) 贊助狀態與作者頁 | `.ttpack` 丟進「文件/TableTavern」解鎖、刪檔還原；作者頁與 +5 配色一起看過 |

排這梯前先確認該項驗收步驟裡沒有換幕：換幕一定走模型產前情提要摘要（`advance_scene`），避不開。

### 跳動記號怎麼捏

`detect_jumps` 只在模型回報新值時比對，且門檻是「絕對幅度 ≥30 **且** 佔較大值四成」，正常玩不保證撞得到。前端只認 `jumps` 這張表有沒有該路徑（[WorkspaceHeader.tsx:185](../../src/views/WorkspaceHeader.tsx)），直接寫進去就會畫記號：

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
| 5 | [refactor-mode-split](../handoffs/refactor-mode-split.md) 五卡矩陣＋同卡連跑三次 | **擋下游最多**：[refactor-card-png-export](../tasks/refactor-card-png-export.md) 待開工首包（套用映射持久化）與 [interface-takeover-spike](../tasks/interface-takeover-spike.md) 逐型驗卡都疊在這條路上。程式碼 2026-08-14 才寫完，出問題時記憶最新、最好修 |
| 6 | [ai-card-refactor](../handoffs/ai-card-refactor.md) B 段→A 段 ＋ [person-promote](../handoffs/person-promote.md) ＋ [state-values-mvu](../handoffs/state-values-mvu.md) 真桌 | 三案一鏈，跑一輪同時收。**前置已解除**：`refactor-outcome-export` 已於 2026-08-11 結案，B 段可直接真跑 orc-cave 卡；產物存檔後 A 段走零額度重放，額度只花一次 |
| 7 | [ai-table-generator](../handoffs/ai-table-generator.md) 一句話開桌 | 六項一輪跑完：開視窗→生成大綱→重骰→改大綱→AI 生成角色→照大綱開桌；順手驗單人設定不錨定角色數、換語言後生成跟著換 |
| 8 | [sponsor-features](../handoffs/sponsor-features.md) AI 生圖 | 三個來源各實跑一次＋構圖二選一（選「半身」要出腰以上特寫、2:3 不變、記住上次選擇）；失敗不扣次數 |
| 9 | [ui-overhaul](../handoffs/ui-overhaul.md) 實聊 playbill | dialogue 事件要有金鑰實聊才出現，串流打字指示改版後沒實測過；可搭任一梯 2 項目順手看 |
| 10 | [worldbook-card-import](../handoffs/worldbook-card-import.md) 篇幅與配角解禁 | **需重新打包**（已裝的 0.2.0 仍是舊行為）：同一張世界書卡確認 GM 旁白篇幅放開、配角會開口、角色回覆有內心戲 |

## 梯 3：等外部條件，不排時程

機會來了順手做，不佔排程。

| 項目 | 卡在哪 |
|---|---|
| [refactor-survey-spans](../handoffs/refactor-survey-spans.md) T4 ② ＋ [refactor-dispatch](../handoffs/refactor-dispatch.md) P8 | 要真的用 API 模式跑一次才看得到 jsonl lane；CLI 模式測不到 |
| [prompt-cache-optimization](../handoffs/prompt-cache-optimization.md) grok／agy 顯示驗收 | 等哪天用到那兩條 lane；OpenRouter 計量未接 |
| [i18n-more-languages](../handoffs/i18n-more-languages.md) | 2026-08-17 拍板延到全 app 功能定案後一次驗，原驗收單已過期 |
