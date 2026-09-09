# Task
Task-ID: free-player-onboarding
Title: 免費玩家零門檻開始：OpenRouter 一鍵連接 → 智慧免費 → 推薦／限時免費模型
Status: in-progress
Created: 2026-09-09T16:21:41+08:00
Updated: 2026-09-09T19:49:00+08:00

## Summary
核心產品目標只有一句：**讓沒有 API 經驗、也不打算先付費的玩家，打開 Table Tavern 後可以用最少步驟開始第一段對話，而且之後不用研究免費模型也能維持實用體驗。**

本案固定分成三個有先後順序、可獨立驗收的階段：

1. **OpenRouter 一鍵連接與免 key**：玩家不需要自己去 OpenRouter 建 API key、複製、貼回 TT，也不需要先理解模型或高／中／低檔位。完成 OpenRouter OAuth 授權後，TT 自動取得並沿用現有本機 key 儲存，先用免費 bootstrap 路由準備好可立即對話的模型。
2. **智慧免費模型**：第一階段的 `openrouter/free` 只當暫時 bootstrap；TT 定期取得目前真的免費且可用的模型狀態，再依 TT 可遠端調整的選模 policy 解析成較合適的具體免費模型。手動選模永遠優先，既有對話不因背景刷新而靜默換模，全部失敗才退回 `openrouter/free`。
3. **推薦模型＋限時免費發現**：智慧選模穩定後，再增加給一般玩家看的「Table Tavern 推薦免費模型」與「限時免費模型」清單，以及可略過／不再顯示的新模型提示；不把模型研究重新丟回玩家身上。

這裡的「零門檻」是指**不用懂 API key、不用選模型、不用先付款**；OpenRouter 帳號登入／註冊與 OAuth 授權仍是必要的外部步驟，不宣稱完全免帳號。

舊案 [easy-pay-onboarding](easy-pay-onboarding.md) 的 OAuth 核心併入本案；舊案 [ai-connection-provider-panels](ai-connection-provider-panels.md) 中與 OpenRouter 免費模型推薦有關的部分也併入本案。CLI／provider-specific 設定頁重構則明確延後，不再綁在這條新手路線上。

規格與分階段驗收見 [plans/free-player-onboarding.md](../plans/free-player-onboarding.md)。

## Progress
- 2026-09-09：重新定調為「免費玩家可以無門檻開始」。把原本混在一起的 OAuth、模型推薦、CLI 設定重構、App 內儲值拆開；本案只保留免費 onboarding 主線，CLI／付款仍另案。
- 2026-09-09 第一自然工作段：完成第一階段的 Rust OAuth coordinator。新增隨機 localhost callback、callback 內嵌並驗證 state、5 分鐘逾時、授權碼換 key、把 key 寫回既有 `api_keys["openrouter"]`；前端以 Web Crypto 產生 PKCE S256 verifier/challenge，後端檢查 RFC 形狀後才開授權頁。OAuth 成功時只有在 API 三 tier 全都沒有明確自訂時才把 `best`／`balanced`／`fast` 填成 `openrouter/free`，既有任一 API tier 自訂就完全不碰；另提供同一保存命令給手動 key fallback，避免兩條路徑 bootstrap 行為分岔。
- 第一段收尾複核發現 repo 提交 `Cargo.lock`；為避免只因 PKCE 新增 direct crypto/url crates 卻漏同步 lockfile，改成前端使用平台 Web Crypto、後端沿用 `reqwest::Url`。因此 dependency 只為 localhost listener 啟用既有 tokio 的 `net` feature，`Cargo.lock` 不需變更。
- 第一段新增單元測試：PKCE verifier/challenge 形狀、authorization URL 的 S256 參數、state／取消／缺 code、bootstrap 相容性、config 保留與 key 落盤。
- 2026-09-09 第二自然工作段：`Onboarding.tsx` 已從「註冊 → 儲值 → 建 key → 貼 key」改成單一「連接 OpenRouter」主動作。前端以 Web Crypto 產 PKCE S256，授權成功直接採用後端回傳的實際 `AppConfig`，不再插入模型選擇步驟。
- 手動 key 仍保留，但收進次要 `<details>` fallback；保存改走 `save_openrouter_key`，與 OAuth 共用 key 落盤及 `openrouter/free` bootstrap，不再由前端自行拼 config。
- OAuth browser／timeout／callback/state／取消／network／exchange／save／PKCE／Web Crypto／空 key 全部映射成可行動文案；新增十語系 onboarding 補充字典，並仍統一經既有 `t()` 入口取字串。補充字典放在 `src/i18n/features/`，避免 `check-i18n.mjs` 把它誤認成第 11 個語系檔。
- 第二段新增前端測試：RFC 7636 S256 官方向量、隨機 verifier/challenge 形狀、十語系所有補充 key 非空、所有 OAuth machine code 都能映射到文案、補充字典確實能經全域 `t()` 取值。
- 已複核既有聊天錯誤分類：HTTP 402／429 原本就會落到 `errQuotaApi`，文案提供「稍後再試／看供應商額度／換 AI 來源」，沒有把付款當唯一解法；一般首次 API request/upstream/call failure 也已有分流，因此本階段沒有另改聊天錯誤框架。
- 兩個工作段均嚴守邊界：沒有改 CLI、高中低 UI、provider-specific panel，也沒有加入後續智慧選模／推薦／限時免費模型資料。
- 2026-09-09 本地驗證修正：第一次 `npm run verify` 先在 rustfmt 發現樣式差異並修正；第二次跑到第 6 關 `cargo check` 發現 repo 實際使用 `ulid 3.0.0`，沒有 `Ulid::new()` API，已把 production nonce 與 cfg(test) 暫存目錄兩處都改成 `Ulid::generate()`。
- **2026-09-09 使用者本地 `npm run verify` 七步全綠**：structure、cargo fmt、vitest、i18n、build、cargo check、cargo test 全部通過。第 1 階段核心程式面與機械驗證完成。
- 2026-09-09 第一階段 UI 收尾小包：將原本壓在 AI 分頁最上方的 `API format` 移到一般設定後方、預設收合的「進階：API 相容設定」；`連線方式` 恢復為 AI 分頁第一個主內容。`preferences.api_mode`、Auto／Chat Completions／Responses 判定與舊 config 全部不變，不做 migration。原本硬編碼英文同步改成十語系 feature-local i18n。此小包只改 `SettingsWindow.tsx`、i18n 入口與 API 相容文案，未碰 CLI、高中低、provider-specific panel 或底層 transport。
- **2026-09-09 UI 收尾後再次 `npm run verify` 七步全綠**：structure、cargo fmt、vitest、i18n、build、cargo check、cargo test 全部通過。第一階段目前沒有待修的機械驗證項目。
- **2026-09-09 產品規格再修正**：確認單靠 `openrouter/free` 隨機免費路由不足以構成長期實用體驗，因此原「推薦／限免」拆成第 2、3 階段。第 2 階段先做「智慧免費」：live catalog 驗證免費／可用狀態，TT 遠端靜態 policy 決定優先順序，本機 cache／內建候選／`openrouter/free` 負責降級；既有進行中對話不得因資料刷新靜默換模。第 3 階段才做玩家可見的推薦清單、限免資訊與可關閉提示。

## Next action
- 用全新 config 實機跑真 OpenRouter OAuth：不建立／複製／貼 key、不選模型、不付款，確認授權回 TT 後能直接送第一句並收到回覆。
- 同一輪檢查落盤：`api_keys["openrouter"]` 有 OAuth key；原本三個 API tier 全空時都為 `openrouter/free`；若事先自訂任一 API tier，三檔完全不被 bootstrap 改寫。
- 再做相容性實機：既有 BYOK key 不強迫 OAuth、手動 key fallback 可完成連線、CLI 路線與 UI 無差異；舊 `api_mode` 自訂值展開進階區後仍正確顯示與生效。
- 第 1 階段實機通過後，直接開第 2 階段「智慧免費模型」；先盤點 config／resolver 的最小相容插點，確保能區分「智慧模式」與玩家明確手動 model id，並確認既有對話要在哪個層級 pin 實際模型。
- 第 2 階段穩定後，才開第 3 階段推薦清單／限時免費資訊／新推薦提示。

## Constraints
- **本案不改 CLI。** Claude／Codex／Antigravity／Grok 的安裝、登入、模型選擇、高／中／低呈現全部維持現況。
- **第 1 階段不重構設定頁。** 不做 provider-specific panel、不把高中低搬家、不新增單模型／分級模式 UI；本次只做 `API format` 的顯示層降級，底層行為不變。
- 現有 `tier_models`、角色卡 tier 語意與舊 config 必須相容；bootstrap 只能填補「尚未明確自訂」的 OpenRouter 模型設定，不得靜默覆蓋既有玩家設定。
- 第 2 階段的智慧免費必須可關閉／可被手動選模覆蓋；不得把動態解析出的 model id 假裝成玩家永久手動選擇，也不得因背景刷新讓既有進行中對話頻繁換模。
- 第 2 階段遠端 policy／catalog 失效時，必須依序使用有效 cache／內建 fallback，最後仍可退回 `openrouter/free`；任何推薦資料問題都不能阻擋聊天。
- 第 3 階段的新推薦／限免提示必須可略過並可不再顯示；手動模式只能提示，不得自動改模。
- 手動貼 OpenRouter key 保留作為 fallback／熟手入口，但不再是新手主路徑。
- 本案不建自營伺服器、資料庫、付費 relay 或 App 內儲值；遠端選模資料第一版只用公開靜態 manifest。
- App 內儲值、轉售額度、金流與地區阻擋不屬本案；若未來真的需要，另案重新做商業與合規評估。
