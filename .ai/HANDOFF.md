# 工作線索引

一線一檔於 [handoffs/](handoffs/)，交接＝就地改寫該檔。開工先整份讀該檔。

分類規則：仍需程式施工／規格落地才放「進行中」；施工已完成、只剩可執行環境／使用者實機／外部條件驗收則移到「等實機驗收」，並同步列入[實測佇列](reference/verification-queue.md)。

## 進行中
- [card-mvu-shim](handoffs/card-mvu-shim.md) — 卡片介面沙盒墊 MVU 讀寫變數：包 1 完成待 GUI 驗收；包 2 設計 Sol 同意，待新代理施工 2a
- [interface-card-panel](handoffs/interface-card-panel.md) — 介面卡渲染面板：v1 實機全過；省額度已由介面接管解掉，v2 剩多卡介面切換、離線退路、代送開關
- [interface-takeover-spike](handoffs/interface-takeover-spike.md) — 介面接管 spike：西幻資料槽型已成立，剩其他卡型驗證；舊產殼路線清理已進 main、等西幻卡實測

## 等實機驗收（順序見[實測佇列](reference/verification-queue.md)）
- [quota-insufficient-alert](handoffs/quota-insufficient-alert.md) — AI 請求失敗彈窗＋送出失敗收回玩家句：已進 main，等實機看彈窗與輸入框還原
- [menu-keyboard-webkit](handoffs/menu-keyboard-webkit.md) — ⋯ 選單 macOS WebKit 鍵盤看不到焦點：已進 main，等實機（外觀＋VoiceOver 混用）
- [card-arrival-private-leak](handoffs/card-arrival-private-leak.md) — 回歸事件漏私設：已進 main，等實機看私設只到 GM
- [grok-cache-miss](handoffs/grok-cache-miss.md) — grok 走續聊：GM 線已驗，角色線（一角一線）等實機連玩三輪
- [ui-redesign](handoffs/ui-redesign.md) — 介面整體重新設計：五包已進 main、範例桌詢問已過；剩重構三窗、實聊名牌與打字指示（自 ui-overhaul 併入）、格式轉換更新等觸發條件型對話窗、Windows 等未實機驗項目
- [desktop-update-detect](handoffs/desktop-update-detect.md) — 桌面版 App 內更新與回退：包 1–5 完成、GUI 煙霧測試過；剩兩個真 release 的端對端驗收
- [state-values-mvu](handoffs/state-values-mvu.md) — 狀態欄二期：八包完成、三處面板 2026-10-02 實機過，剩真桌實跑（併在 ai-card-refactor 之後）
- [worldbook-card-import](handoffs/worldbook-card-import.md) — 世界書卡匯入：本地操作 2026-10-02 實機過，剩篇幅／配角解禁實聊排梯 2 第 10
- [sponsor-features](handoffs/sponsor-features.md) — 贊助三項：贊助狀態與作者頁 2026-10-02 實機過，剩 AI 生圖排梯 2 第 8
- [refactor-mode-split](handoffs/refactor-mode-split.md) — 重構雙軌定向：五卡矩陣 2026-10-02 測試包跑過，剩四洞①②④ GUI 重測、③缺合適卡、重構中取消
- [ai-card-refactor](handoffs/ai-card-refactor.md) — AI 卡重構按鈕：產出重設計與匯出重構卡都已結案，前置已解除；等 B 段→A 段並與 person-promote／state-values-mvu 合併真桌驗收
- [person-promote](handoffs/person-promote.md) — AI 認人並合併升格：實作完成四項自驗綠，與 ai-card-refactor 合併實機驗收
- [ai-table-generator](handoffs/ai-table-generator.md) — 一句話開桌：六項一輪跑完，排梯 2 第 7
- [refactor-survey-spans](handoffs/refactor-survey-spans.md) — 盤點四分類＋照搬零輸出：T4 三項過，API 退 GM 檔那項延到真用 API 模式時驗
- [refactor-dispatch](handoffs/refactor-dispatch.md) — AI 重構提速省費：P4–P6 已隨 refactor-survey-spans T4① 過，只剩 P8（API 模式跑重構）
- [prompt-cache-optimization](handoffs/prompt-cache-optimization.md) — 提示詞快取優化：程式面包 1–7 完成、額度分頁全驗過；剩離開提醒自然遇到再看、undo 截尾優化等非必要項，OpenRouter／API 已收束出本案範圍
- [i18n-more-languages](handoffs/i18n-more-languages.md) — 十國語言：機械關卡持續綠，人眼審校改到全 app 功能定案後一次驗
- [api-shared-lane](handoffs/api-shared-lane.md) — API 路徑改走 chars 共線：包 A／B 完成、Sol 過，剩錯認前言者＋四路快取成對測試
- [claude-compat-endpoint](handoffs/claude-compat-endpoint.md) — Claude 相容端點：實作完成、cargo/build 雙驗證綠，等使用者用真相容端點實測後結案
