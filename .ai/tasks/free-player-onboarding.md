# Task
Task-ID: free-player-onboarding
Title: 免費玩家零門檻開始：OpenRouter 一鍵連接 → 推薦／限時免費模型
Status: todo
Created: 2026-09-09T16:21:41+08:00
Updated: 2026-09-09T16:21:41+08:00

## Summary
核心產品目標只有一句：**讓沒有 API 經驗、也不打算先付費的玩家，打開 Table Tavern 後可以用最少步驟開始第一段對話。**

本案固定分成兩個有先後順序、可獨立驗收的階段：

1. **OpenRouter 一鍵連接與免 key**：玩家不需要自己去 OpenRouter 建 API key、複製、貼回 TT，也不需要先理解模型或高／中／低檔位。完成 OpenRouter OAuth 授權後，TT 自動取得並沿用現有本機 key 儲存，並用免費 bootstrap 路由準備好可立即對話的模型。
2. **推薦模型＋限時免費模型**：第一階段穩定後，才增加給一般玩家看的「Table Tavern 推薦免費模型」與「限時免費模型」清單；預設仍保留永遠可退回的免費 bootstrap，不讓推薦資料失效阻斷遊戲。

這裡的「零門檻」是指**不用懂 API key、不用選模型、不用先付款**；OpenRouter 帳號登入／註冊與 OAuth 授權仍是必要的外部步驟，不宣稱完全免帳號。

舊案 [easy-pay-onboarding](easy-pay-onboarding.md) 的 OAuth 核心併入本案；舊案 [ai-connection-provider-panels](ai-connection-provider-panels.md) 中與 OpenRouter 免費模型推薦有關的部分也併入本案。CLI／provider-specific 設定頁重構則明確延後，不再綁在這條新手路線上。

規格與分階段驗收見 [plans/free-player-onboarding.md](../plans/free-player-onboarding.md)。

## Progress
- 2026-09-09：重新定調為「免費玩家可以無門檻開始」。把原本混在一起的 OAuth、模型推薦、CLI 設定重構、App 內儲值拆開；本案只保留前兩者，且 OAuth 必須先獨立完成。

## Next action
- 先做第 1 階段：依 OpenRouter 當下官方 OAuth PKCE 規格完成授權 → 換 key → 寫入既有本機設定 → 用 `openrouter/free` 作為新安裝／未自訂設定時的免費 bootstrap → 回到桌面即可送出第一句。
- 第 1 階段實機通過後，才開第 2 階段推薦／限時免費模型清單。

## Constraints
- **本案不改 CLI。** Claude／Codex／Antigravity／Grok 的安裝、登入、模型選擇、高／中／低呈現全部維持現況。
- **第 1 階段不重構設定頁。** 不做 provider-specific panel、不把高中低搬家、不新增單模型／分級模式 UI。
- 現有 `tier_models`、角色卡 tier 語意與舊 config 必須相容；bootstrap 只能填補「尚未明確自訂」的 OpenRouter 模型設定，不得靜默覆蓋既有玩家設定。
- 手動貼 OpenRouter key 保留作為 fallback／熟手入口，但不再是新手主路徑。
- 第 1 階段不建自營伺服器、資料庫、付費 relay 或 App 內儲值。
- 第 2 階段的遠端推薦資料失效時，`openrouter/free`／內建 fallback 仍必須讓玩家能玩。
- App 內儲值、轉售額度、金流與地區阻擋不屬本案；若未來真的需要，另案重新做商業與合規評估。
