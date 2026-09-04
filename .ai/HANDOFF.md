# 工作線索引

一線一檔於 [handoffs/](handoffs/)，交接＝就地改寫該檔。開工先整份讀該檔。

## 進行中
- [api-cache-visibility](handoffs/api-cache-visibility.md) — API 路快取看得見：實作＋Sol 驗收完成，等真跑一輪 api 對話看命中率顯示
- [api-key-paste-guard](handoffs/api-key-paste-guard.md) — 金鑰貼錯防呆：功能面完成、實機兩項過，剩換上真金鑰後自然驗到
- [grok-profile-isolation](handoffs/grok-profile-isolation.md) — grok 環境隔離：四處注入完成、Sol 過，剩設定頁跑一次 grok 登入
- [refactor-survey-spans](handoffs/refactor-survey-spans.md) — 盤點四分類＋照搬零輸出：T4 三項過，API 退 GM 檔那項延到真用 API 模式時驗
- [refactor-mode-split](handoffs/refactor-mode-split.md) — 重構雙軌定向：包 1–3 實作完，剩包 4 的五張卡實機驗收矩陣
- [refactor-ai-split](handoffs/refactor-ai-split.md) — production 9 模組已接線、legacy 僅 `cfg(test)`；下一步＝搬測試、刪 legacy、跑最終驗收
- [refactor-dispatch](handoffs/refactor-dispatch.md) — AI 重構提速省費：提速與品質已移交 refactor-survey-spans，剩取消類 P4–P6／P8 合併驗
- [ai-card-refactor](handoffs/ai-card-refactor.md) — AI 卡重構按鈕：七包完成、實測三 bug 已修，等 refactor-outcome-export 後跑 B 段
- [person-promote](handoffs/person-promote.md) — AI 認人並合併升格：實作完成四項自驗綠，與 ai-card-refactor 合併實機驗收
- [state-values-mvu](handoffs/state-values-mvu.md) — 狀態欄二期：八包完成 cargo 317 綠，等真桌實跑（併在 ai-card-refactor 之後）
- [prompt-cache-optimization](handoffs/prompt-cache-optimization.md) — 提示詞快取優化：包 1–7 實機驗收過，剩 grok／agy 顯示驗收與 OpenRouter 計量
- [interface-card-panel](handoffs/interface-card-panel.md) — 介面卡渲染面板：v1 實機全過，v2 首要＝省額度（歷史裡整包 XML 重送）
- [i18n-more-languages](handoffs/i18n-more-languages.md) — 十國語言：機械關卡持續綠，人眼審校改到全 app 功能定案後一次驗

## 等實機驗收（順序見[實測佇列](reference/verification-queue.md)）
- [worldbook-card-import](handoffs/worldbook-card-import.md) — 世界書卡匯入：本地那批排梯 1 第 3，篇幅／配角解禁需重新打包排梯 2 第 10
- [sponsor-features](handoffs/sponsor-features.md) — 贊助三項：贊助狀態與作者頁排梯 1 第 4，AI 生圖排梯 2 第 8
- [ai-table-generator](handoffs/ai-table-generator.md) — 一句話開桌：六項一輪跑完，排梯 2 第 7
- [ui-overhaul](handoffs/ui-overhaul.md) — Emblem 設計系統：深色已驗過，剩實聊 playbill 與串流打字指示，排梯 2 第 9
