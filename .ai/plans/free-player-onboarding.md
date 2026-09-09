# 免費玩家零門檻開始：OpenRouter 一鍵連接 → 推薦／限時免費模型

本檔存放 [free-player-onboarding](../tasks/free-player-onboarding.md) 的正式規格。2026-09-09 重新整理自舊 `easy-pay-onboarding` 與 `ai-connection-provider-panels`；目標是把「先能玩」與「之後把模型選擇做漂亮」拆成兩個階段，不再把 CLI／設定頁大改綁進新手 onboarding。

## 產品目標

新玩家不需要先知道 API key、model id、provider、best／balanced／fast 是什麼，也不需要先付款。正常路徑應該是：

```text
打開 Table Tavern
→ 進入桌面／範例桌
→ 需要 AI 時按「連接 OpenRouter」
→ 瀏覽器登入或註冊 OpenRouter 並授權
→ 回到 Table Tavern
→ 直接開始對話
```

「零門檻」不等於「零外部帳號」：OpenRouter 帳號與 OAuth 授權仍存在，只是玩家不再手工建立／複製／貼上 key，也不需要在第一句話之前研究模型。

---

# 第 1 階段：OpenRouter 一鍵連接與免 key

## 1. 範圍

本階段只解決一件事：**從「還沒有 OpenRouter 連線」到「可以送出第一句並收到 AI 回覆」之間，不要求玩家碰 API key 或模型設定。**

### 1.1 OAuth PKCE

依 OpenRouter 開工當下的官方 OAuth PKCE 文件實作，不把歷史 endpoint／參數當永遠不變的常數。預期流程：

1. App 建立 PKCE verifier／challenge 與 `state`。
2. 用系統瀏覽器開 OpenRouter 授權頁。
3. 玩家登入／註冊並核准 Table Tavern。
4. callback 回到本機 App。
5. App 以授權碼交換可用 API key。
6. key 寫入現有 `api_keys["openrouter"]` 本機儲存；不另造帳號系統或後端。

必須驗證 `state`，取消、逾時、callback 失敗與交換 key 失敗都要能安全返回，不留下半套連線狀態。

### 1.2 免費 bootstrap：先用 `openrouter/free`

第 1 階段**不做模型推薦系統**。目前優先 bootstrap 候選使用 OpenRouter 官方的 `openrouter/free` 免費路由：它的用途就是在可用免費模型間選路由，適合讓新玩家先開始。

為了不動現有 tier 架構，最小相容策略是：

- 新安裝／沒有明確自訂 OpenRouter 模型時，讓 API 的 `best`／`balanced`／`fast` 都能解析到 `openrouter/free`；實作可採「第一次連線時填入既有 `tier_models`」或「resolver 對空值給 bootstrap fallback」，開工時以最少改動與最好測試性二選一。
- `gm_tier` 與角色卡既有 tier 語意不改。
- 已有明確 OpenRouter tier 設定的玩家完全不覆蓋。
- 已有 BYOK OpenRouter key 的玩家也不強迫重跑 OAuth。

這一步的目的不是宣稱 `openrouter/free` 是最佳 RP 模型，而是確保**不用選模型也能免費送出第一句**。

### 1.3 新手入口

目前 onboarding 的「註冊 → 儲值 → 建 key → 貼 key」改成單一主動作，例如「連接 OpenRouter」。

要求：

- 主路徑不要出現 API key、endpoint、model id、高／中／低等術語。
- 不要求玩家先進「設定 → AI 連線」研究欄位；從桌面／onboarding 就能發起連接。
- OAuth 成功後直接回到可對話狀態，不再插入模型選擇步驟。
- 手動貼 key 保留為次要 fallback／熟手入口，不刪功能。

### 1.4 錯誤與降級

至少處理：

- 玩家取消 OAuth。
- callback／`state` 不符、逾時、瀏覽器流程未完成。
- 網路錯誤或交換 key 失敗。
- key 取得成功但首次請求失敗。
- 免費路由暫時不可用或碰到 rate limit。

錯誤訊息只告訴玩家下一步要做什麼；不要把內部 key 或 OAuth 細節丟給一般玩家。免費路由暫時不可用時可以提供重試／稍後再試／手動選模型等出口，但不把「先付款」當唯一解法。

## 2. 第 1 階段明確不做

- 不改 Claude／Codex／Antigravity／Grok CLI。
- 不拆 provider-specific panel。
- 不移動或隱藏現有高／中／低 UI。
- 不做推薦模型清單、限時免費清單、GitHub manifest。
- 不做 GitHub Actions 自動更新模型資料。
- 不做 App 內儲值、relay、轉售額度、付款 webhook 或地區封鎖。

## 3. 第 1 階段驗收

### 新玩家實機

用全新 config 驗收：

1. 沒碰過 API 的玩家打開 TT。
2. 不建立、不複製、不貼任何 API key。
3. 不選 model id、不配置高中低。
4. 不先付款。
5. 完成 OpenRouter 登入／註冊與 OAuth 授權後，回到 TT。
6. 能送出第一句並收到模型回覆。

### 相容性

- 既有 OpenRouter key 仍可用。
- 既有 `tier_models` 不被覆蓋。
- CLI 路線行為與 UI 無差異。
- 手動 key fallback 仍可完成連線。

### 自動測試最低項

- PKCE／`state` 驗證與錯誤路徑。
- OAuth 成功後 key 落到既有 config 的正確欄位。
- bootstrap 只填空缺，不覆蓋既有 tier。
- 未自訂 tier 時 API 三檔都能解析到免費 bootstrap。

第 1 階段完成、實機真的做到「免 key 免費第一句」後，才進第 2 階段。

---

# 第 2 階段：推薦模型＋限時免費模型

## 4. 目標

第 1 階段解決「能不能玩」；第 2 階段才解決「免費玩家怎麼知道有哪些更好的選擇」。

一般玩家應能看到兩類簡單清單：

1. **Table Tavern 推薦免費模型**：針對 RP／長對話等 TT 使用情境人工策展。
2. **限時免費模型**：原本付費、活動期間免費或具有明確免費期限／免費活動的模型；只有資料來源足夠可信時才標示。

`openrouter/free` 繼續當安全底線：推薦服務掛掉、斷網、manifest 壞掉、某模型下架，都不能讓免費新玩家失去可用預設。

## 5. 資料來源分工

### 5.1 OpenRouter catalog：客觀現況

沿用現有 `/api/v1/models` 清單，開工時只增加 UI 真正需要的最小 metadata，例如：

- 是否零價格／免費 variant。
- model id／顯示名。
- 必要時的 context length／能力欄位。
- 若官方確實提供且可靠，再讀 expiration／活動相關欄位。

Catalog 回答「現在有什麼、現在是否免費」，不直接代表 Table Tavern 推薦。

### 5.2 Table Tavern 靜態 manifest：主觀推薦

第一版不架自營伺服器。公開 repo 放一份小型靜態 manifest，由 App 背景抓取；至少能表達：

- schema/version。
- `updated_at`。
- 推薦免費模型的順序與短理由。
- 限時免費項目的標籤；若有可信期限則可附 `expires_at`。
- 暫時停薦／排除。

遠端 manifest 只負責策展，不保存玩家資料、不代理模型流量。

### 5.3 本機 fallback

發行版內建最小 fallback。遠端資料抓不到時：

1. 仍可用 `openrouter/free`。
2. 可顯示最後一次成功快取的推薦資料，但必須標示資料時間，避免把過期活動當現在仍有效。
3. 不因推薦資料問題阻擋聊天。

## 6. 第 2 階段 UI 原則

這一輪仍**不做整個 provider 設定頁重構**。只在 OpenRouter 的一般玩家路徑上增加必要的模型發現介面。

可以呈現為：

```text
目前使用
OpenRouter 免費自動選擇

Table Tavern 推薦免費模型
- Model A　推薦：角色扮演
- Model B　推薦：速度快

限時免費
- Model C　免費至 2026-xx-xx（只有可信期限才顯示）
```

選一個模型後如何映射到現有三 tier，開工時以「不靜默覆蓋熟手既有自訂」為最高原則；不為了這個清單順手重做高中低模型系統。

## 7. 第 2 階段明確不做

- 不改任何 CLI provider 的設定方式。
- 不把 Claude／Codex／Antigravity／Grok 改成 provider-specific panel。
- 不為了模型清單刪除既有 tier contract 或做 config migration。
- 第一版不做 GitHub Actions 自動策展；推薦仍是人工決定。
- 不做 App 內儲值或任何自營付費服務。

## 8. 第 2 階段驗收

- 免費玩家不用打開完整模型目錄，也看得懂「目前能免費用什麼」。
- 推薦免費與限時免費有清楚區別。
- 限時免費沒有可靠證據就不標期限、不猜。
- 選推薦模型後可正常聊天。
- 推薦服務斷線／資料過期／推薦模型下架時，仍會退回可用免費 bootstrap，而不是卡死。
- 舊 OpenRouter 熟手設定與全部 CLI 路線不受影響。

---

# 延後另案

以下全部從本案移出：

- `ai-connection-provider-panels`：各供應商專屬下半頁、CLI 是否顯示單模型或預設模型、高中低 UI 搬家。
- App 內儲值／自營 relay／金流／地區阻擋。
- 模型推薦的自動化營運（GitHub Actions、後端服務、推播等）。

只有前兩階段真的被玩家使用後，再依實際痛點決定要不要開這些案。
