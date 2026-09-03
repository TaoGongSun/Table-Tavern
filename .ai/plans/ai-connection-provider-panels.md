# AI 連線設定重整：OpenRouter 免費推薦＋供應商專屬面板

本檔存放 [ai-connection-provider-panels](../tasks/ai-connection-provider-panels.md) 的規格細節，由任務檔 Summary 連回。2026-09-03 立案；以下為本次討論已拍板的方向，未拍板細節留到開工時依現況實作，不在立案階段過度設計。

## 問題定調（2026-09-03）

目前 AI 連線頁已把 OpenRouter、Claude、Codex、Antigravity、Grok 放在同一頁，但下半部仍沿用「所有 transport 都要配置高／中／低模型」的共同表單。這會讓不同供應商被迫套進同一種抽象，尤其 CLI 路線看起來比實際需要更複雜。

新的產品方向不是再造一個「進階設定」頁，而是保留所有連線方式同頁可見：玩家選哪個 provider，下方整塊內容就換成該 provider 真正需要的設定。複雜功能只在自身區塊裡按需展開，不用「進階使用者」標籤把人先分類。

## 已拍板 UX 原則

### 1. 所有連線方式平等顯示

AI 連線頁頂部繼續同時顯示：

- OpenRouter
- Claude
- Codex
- Antigravity
- Grok

不新增「基本／進階」總分頁，不把 CLI 整組藏起來。未安裝、未登入、已連線等狀態照 provider 顯示在自己的選項旁。

### 2. 下半頁改為 provider-specific panel

選 OpenRouter 時，只顯示 OpenRouter 需要的內容；選 Claude／Codex／Antigravity／Grok 時，只顯示各自的安裝、登入、重新驗證、換帳號、模型選擇等真正有意義的控制。

跨 provider 的遊戲行為設定（例如每輪最多發言角色數）獨立放在共同區，不與模型選擇混在一起。

### 3. OpenRouter 預設路徑＝一個推薦免費模型

一般玩家選 OpenRouter 後，預設只需要理解一件事：

> Table Tavern 已替你選好目前推薦的免費模型。

第一層畫面應盡量接近：

```text
OpenRouter
✓ 已連線

目前模型
★ Table Tavern 推薦免費模型
<Model name>

[顯示其他模型]
```

首次使用者不必先理解 `best`／`balanced`／`fast`、模型供應商、價格表或完整 model id 才能開始玩。

### 4. 高／中／低保留，但預設隱藏

既有 tier 系統不刪除。`best`／`balanced`／`fast` 仍可用於讓重要角色、一般角色、輕量角色走不同模型，也保留 GM tier。

但這些控制收進 OpenRouter 的「顯示其他模型」展開區，不再是預設第一屏。展開後可包含：

- 其他免費模型
- 付費模型
- 自訂 model id
- 高／中／低模型分級
- GM 使用哪個 tier
- 自訂 OpenAI-compatible base URL（若保留在 OpenRouter/API 區）

未碰展開區的玩家視為「單模型模式」：GM 與所有角色直接使用目前推薦／選定模型。只有玩家明確配置 tier 後才進入自訂分級玩法。

實作時可沿用既有 `tier_models` 資料，避免破壞舊 config 與角色卡；是否新增一個明確的「單模型／分級模式」旗標，開工時再依最小相容改動決定。

### 5. OpenRouter 預設免費，但不鎖死免費

OpenRouter 的產品定位是「最容易免費開始玩的 API 入口」，不是「只能使用免費模型」。

預設推薦清單優先把免費模型放最前面；玩家展開後仍能選：

- 其他免費模型
- 付費模型
- 自訂模型

因此不移除既有 OpenRouter 完整模型能力。

### 6. CLI 不強迫套高／中／低

CLI provider 不再因為底層有 tier 欄位，就一律顯示三組模型選單。每家只呈現自己有意義的模型選擇方式：

- 能／值得選單一模型：顯示單一模型控制。
- CLI 本身有清楚且穩定的模型選項：列出該 CLI 實際支援的選項。
- 不值得由 Table Tavern 指定：直接使用 CLI 預設模型，不額外製造設定。

既有 `claude:best`、`codex:best` 等舊 config 第一階段可保留但不再強迫 UI 使用；不要為了清資料增加 migration 風險。

Claude 的 Anthropic-compatible base URL／key 這種少數功能可以繼續用局部 `<details>`，名稱描述實際用途（例如「使用其他 Claude 相容服務」），不統稱「進階設定」。

## OpenRouter 推薦資料來源

### 目標

需要同時解決兩件不同的事：

1. OpenRouter 現在客觀有哪些模型、哪些免費、哪些已下架／改價。
2. Table Tavern 主觀推薦哪些模型給一般 RP 使用者。

兩者不可混成同一來源。

### A. OpenRouter 公開模型目錄（客觀現況）

現有 app 已直接抓 `https://openrouter.ai/api/v1/models` 並快取；第一階段沿用，不新增 proxy server。

目前 parser 只留 `id`／`name`。開工時要評估保留判斷推薦畫面所需的最小 metadata，例如：

- pricing（判斷免費／付費）
- context length（若 UI／篩選需要）
- expiration date（若官方資料有提供且實際可靠）
- 其他真的會用到的能力欄位

不要為了「可能以後有用」把整份 API response 原樣塞進 app 型別。

### B. Table Tavern 推薦 manifest（主觀策展）

第一階段不架自營伺服器，直接在公開 repo 放一份靜態 JSON，例如：

```text
remote/openrouter-recommendations.json
```

App 可從 GitHub raw／日後 GitHub Pages 讀取。manifest 至少表達：

- schema/version
- updated_at
- 推薦免費模型的優先順序
- 可選的推薦理由／短標籤
- 值得提示的限時免費／活動（僅在確認資料可信時）
- 必要時的暫時排除／停薦

具體 schema 開工時先以最小可用格式定稿，不在立案時把未來欄位做滿。

### C. 內建 fallback

發行版內建一份推薦 fallback。啟動時可依序：

1. 立即用內建推薦與本機 catalog 快取顯示畫面。
2. 背景抓 OpenRouter 最新 catalog。
3. 背景抓 GitHub 最新推薦 manifest。
4. 成功就合併更新；任一來源失敗都不阻擋設定頁與遊戲。

遠端推薦服務故障不得變成 Table Tavern 的單點故障。

## GitHub 是第一階段的「0 → 1」遠端基礎設施

本案第一階段刻意不新增 Cloudflare Worker、VPS、資料庫或自營 API。

公開 GitHub repo 已存在，因此先把它同時當：

- 可更新的推薦資料來源
- 版本歷史
- 靜態檔託管

後續若真的有必要，再考慮 GitHub Pages 或其他 CDN；不是開工前提。

### GitHub Actions（第二階段候選，不是第一包必做）

未來可用 Actions 定時抓 OpenRouter catalog，產生純客觀的 generated 資料，例如：

- 新增／消失的免費模型
- 價格變化
- expiration 資料變化

但 Actions 不應自行把「新免費模型」升格成「Table Tavern 推薦」。推薦仍屬人工／半人工策展判斷。

## 與 easy-pay-onboarding 的關係

[easy-pay-onboarding](../tasks/easy-pay-onboarding.md) 原本規劃 OpenRouter OAuth PKCE，目標是把「註冊 → 建 key → 貼 key」進一步縮成「連接 OpenRouter」。這個方向不衝突，日後可直接接到本案的 OpenRouter panel。

本案先處理 provider 面板與推薦模型架構；OAuth 是否同一輪實作由排程決定。無論 OAuth 何時做，今天已拍板：

- CLI／BYOK 不再整體收進「進階」摺疊。
- OAuth 成功後應落到同一個 OpenRouter 面板，預設選推薦免費模型，而不是再要求玩家先配置三個 tier。

## 建議分包

### 包 1：provider-specific UI 骨架

- 保留頂部所有 provider 選項。
- 把目前混在同一 form 的 OpenRouter／CLI 模型區拆成 provider-specific rendering。
- CLI 先取消預設三 tier UI；每家依現有 catalog 能力顯示單一／預設模型控制。
- 跨 provider 的共同設定整理到獨立共同區。
- 不改遠端推薦、不改 OAuth。

### 包 2：OpenRouter 單模型預設＋展開式完整模型設定

- 新增「目前模型／推薦免費」第一層。
- 「顯示其他模型」展開後才出現完整免費／付費／自訂選擇與 tier 配置。
- 舊 `tier_models` 相容；確認未展開／未自訂時所有角色與 GM 的模型路由定義一致。

### 包 3：推薦 manifest＋catalog metadata

- 新增 repo 內推薦 JSON 與 app parser。
- 既有 OpenRouter model parser 保留最小必要 metadata。
- 內建 fallback＋GitHub remote＋OpenRouter catalog 合併。
- 斷網、GitHub 失敗、manifest 壞掉、模型已下架都要有合理 fallback。

### 包 4：首次引導整合

- Onboarding 從「去官網 → Keys → 貼 key」逐步靠向新的 OpenRouter panel。
- 若 easy-pay-onboarding 的 OAuth 同期開工，接 OAuth；否則先保留手動 key，但完成後直接使用推薦免費模型，不再要求配置 tier。
- 重新檢查 onboarding 文案中把 CLI 稱作「進階使用者」的文字，改成中性描述。

### 包 5：遠端自動化（條件式）

只有前四包實際使用後證明需要更即時的免費模型／活動更新，才加 GitHub Actions。此包不是 MVP 前提。

## 驗收原則

### 一般新手

- 沒有 AI API 經驗的玩家能看見所有連線選項，但不會被「進階」標籤嚇退。
- 選 OpenRouter 後，第一屏不需要理解高／中／低，就能知道目前會使用哪個推薦免費模型。
- 不打開「顯示其他模型」也能正常遊玩。

### OpenRouter 熟手

- 能展開看到其他免費模型、付費模型、自訂 model id。
- 能繼續配置高／中／低與 GM tier。
- 舊設定不因新 UI 消失或被無聲覆寫。

### CLI 使用者

- 選 Claude／Codex／Antigravity／Grok 時，不會看到與該 CLI 無關的 OpenRouter API key／base URL／三 tier 欄位。
- 安裝、登入、重新驗證、換帳號等既有能力不退化。
- 每家模型控制只顯示其實際有意義的選項。

### 離線／服務失敗

- GitHub 推薦檔抓不到：使用內建 fallback。
- OpenRouter catalog 抓不到：沿用既有快取；必要時仍可手動 model id。
- 遠端 manifest 有未知欄位／未知 model：忽略或降級，不讓設定頁崩潰。

## 不在本案第一階段處理

- 自營 relay／後端帳號系統。
- App 內代收 OpenRouter 費用。
- 以伺服器代管玩家 API key。
- 自動 benchmark 所有新模型並自行決定推薦順位。
- 為了新 UI 立即刪除所有舊 CLI tier config。

## 待開工時確認的小問題

以下不影響立案，實作時再依現有程式最小改動決定：

1. 「顯示其他模型」展開區是直接列免費／付費分組，還是先只列推薦＋搜尋。
2. 單模型模式在資料層要新增明確旗標，或以「未自訂 tier」推導。
3. 各 CLI 哪些目前真的支援穩定的模型清單／單一選模，哪些應直接用 CLI default。
4. GitHub raw 是否直接作正式 URL；若遇到快取／CORS／可用性問題再切 Pages。
