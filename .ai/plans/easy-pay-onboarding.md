# 【已拆案】簡易付費入口／OpenRouter OAuth 舊規劃

本規格於 2026-09-09 被 [free-player-onboarding](../tasks/free-player-onboarding.md) 取代。

保留的核心決策只有：OpenRouter 新手主路徑應使用 OAuth PKCE，讓 App 自動取得並保存使用者自己的 key，而不是要求玩家自己去 Keys 頁建立、複製、貼回 Table Tavern。這部分已移到新規格的第 1 階段。

原規格中的「App 內儲值 → 自營 relay → 金流／地區阻擋」已退出目前 roadmap。若未來有真實需求，必須另立新案並重新查當時的 OpenRouter、支付商與地區合規條款，不能把本檔舊研究直接當成可施工規格。

歷史細節仍可由 Git history 回看；本檔不再作實作依據。
