# KICKOFF — Story Table 工程起手文件

> 文件日期：2026-07-18
> 讀者：從零開始實作本專案的全新 Claude 對話。產品規格見 `NewPlan.md`（本檔不重複其內容，衝突時以 NewPlan.md 為準）。
> 本 repo 已清場：除 NewPlan.md 與本檔外沒有任何舊碼。不要去找、也不要參考任何舊實作或上游專案。

---

## 1. 技術棧（拍板，使用者可否決）

- **Tauri 2**（Rust 後端）＋ **Vite + React + TypeScript** 前端。
- 為何選它：免執行時依賴（使用者拖進 Applications 雙擊即用，符合 NewPlan §4 極簡安裝）、用系統 WKWebView 體積小、web 前端自由度符合 §9 的視覺設計、Rust 端原生處理檔案 IO／HTTP 串流／子程序（CLI 傳輸）。
- 備選（若使用者否決）：Python 本機 server＋WKWebView 殼——開發最快，但散布需要使用者有 python3，違反 §4 的極簡安裝承諾。
- 打包：`tauri build` 產 DMG，ad-hoc 簽章即可（不上架、不公證；README 需附 Gatekeeper 的右鍵開啟／xattr 說明）。

## 2. Repo 佈局草案

```text
Story-Table/
├── NewPlan.md / KICKOFF.md
├── src/            # React 前端
├── src-tauri/      # Rust 後端（指令、傳輸層、檔案存取）
└── docs/           # 決策紀錄（隨開發新增）
```

## 3. 資料落地格式（NewPlan §5.1 的落地細節）

使用者資料放 `~/Documents/StoryTable/worlds/<世界名>/`（可在設定改路徑）：

- `world.md`：世界書 v1，純 Markdown，整份作為 GM 的 system prompt 素材（§7.0）。
- `characters/<角色名>.md`：YAML frontmatter ＋ Markdown 內文。frontmatter 欄位：`name`、`color`（hex，建卡時從調色盤自動配）、`avatar`（相對路徑或內建 emoji）、`tier`（`best`/`balanced`/`fast`/`default`）。內文分節：`## 公開`（人格、語氣、背景，所有人可見的部分）與 `## 私有`（秘密、私人目標、私人記憶——只進本角色與 GM 的上下文）。
- `transcript/<場景序號>.jsonl`：每行一個事件 `{ts, speaker, kind, text}`；`kind` ∈ `dialogue`（角色對話）/`narration`（GM 旁白）/`player`（玩家輸入）/`system`（點名、進離場）。
- `state.json`：執行期狀態——角色↔傳輸層/模型對應、場景指標、離場角色的補課摘要。角色卡永不寫入供應商資訊（§5.1）。
- 全域設定 `~/Library/Application Support/StoryTable/config.json`：API key（檔案權限 0600，與各家 CLI 儲存憑證的慣例相同）、檔位→模型對應表、偏好。

## 4. 傳輸層（NewPlan §8.1 的落地細節）

兩種傳輸共用同一個「上下文組裝→單發呼叫→串流回傳」介面：

### 4.1 API 直連（先做）

- 預設 OpenRouter：`POST https://openrouter.ai/api/v1/chat/completions`，OpenAI-compatible，SSE 串流。
- 檔位預設對應（進階設定可改）：`best`→高階旗艦、`balanced`→sonnet 級、`fast`→輕量模型；具體 model id 實作時查 OpenRouter 目錄現況再定，寫成設定檔不寫死在程式裡。
- 進階設定允許自訂 base URL（天然支援 Ollama 等 OpenAI-compatible endpoint，§3.3）。
- Prompt caching：MVP 不特別處理，列為後續優化。

### 4.2 CLI 傳輸（後做，MVP 第 3 項）

- 原則照 NewPlan §3.2／§4.2／§8.1：只偵測不代辦、啟用前風險告知、headless 單發＋system prompt 覆寫、不依賴 CLI 自身 session。
- 已知線索（**實作時一律以當場 `--help` 查證為準，勿信本段快照**）：Claude Code 有 `claude -p`（headless）、`--append-system-prompt`、`--output-format stream-json`、`--model`；Codex 有 `codex exec -m <model>`、`-c model_reasoning_effort=<值>`。
- 偵測方式：`which` ＋常見安裝路徑掃描；顯示版本供使用者確認。

## 5. MVP 施工順序（切片，每片可獨立驗收）

1. **資料層**：建世界／讀寫角色卡與 world.md／transcript 追加寫入。驗收：檔案格式與本檔 §3 一致，App 重啟後狀態還原。
2. **API 傳輸＋單角色對話**：貼 OpenRouter key、跟單一角色對話、串流顯示。驗收：上下文只含該角色可見內容。
3. **群聊室 UI**：多角色泡泡（顏色／頭像／逐角色打字中指示）、GM 旁白區塊、玩家輸入、手動點名（§9.2）。
4. **簡易導演**：GM 上下文含 world.md＋全部公開歷史；可選下一位發言者、插入旁白（§6.1）。
5. **CLI 傳輸**：偵測＋風險告知＋headless 呼叫（本檔 §4.2）。
6. **Onboarding**：BYOK 引導流程（§4.1），含費用直覺化文案。
7. **打包**：DMG＋Gatekeeper 說明＋最小 README。

## 6. 禁止事項

- 不參考、不引用 collab board 或任何上游專案的程式碼與設計（repo 已清場，也不要去網路上找）。
- 不做規避偵測功能（NewPlan §4.3）、不加遙測、不連任何非使用者設定的外部服務。
- 不提前實作 NewPlan §12「第一版暫時不做」清單內的功能。
- 新增依賴前先問：標準庫或已有依賴能不能做？（能就不加）
