# 2026-10-06 夜間連跑總表

作者授權（2026-10-06 晚）：照順序一路派子代理做到結案，卡在要作者決定的先擱置跳下一案。每案都走「子代理施工 → Sol＋Grok 審 → 主線讀 diff、重跑 verify → 兩邊驗收 → 子代理整理歷史合併」。

## 已結案進 main

| 案 | commit | 重點 |
|---|---|---|
| grok-cache-miss | 9c8713a | grok 角色線續聊驗收；偶發整線不命中記為已知 |
| grok-system-override | a21eeda | grok 呼叫帶 `GROK_CAMPAIGNS=0`：遠端 campaign 讓 `-m` 非預設模型時丟掉 app 的 system |
| grok-shared-lane | e5815af | grok 角色改全角色共線，回合後抹 session 兩檔的私設與 reasoning；同桌 lane 呼叫與保溫用每桌 mutex 串行 |
| grok-catalog-parse | 84a57f7 | 設定頁 grok 模型下拉列得出全部模型 |
| usage-cache-audit | 69972bb、3f534c1、b000da7、2d4f3b5 | 昨晚另一串做大半，今晚接手補 grok 對帳並結案；丟線帳本 transport 記實際 provider；grok 換 session 自救「本次樣本下暫不做」〔模型判斷·未裁決〕 |
| api-stream-stall-timeout | 4f82e80 | API 串流停滯逾時（首字 300 秒、之後 120 秒，保活註解不續命） |
| harness-iframe-shot | df6ecb1 | 測試包關掉 WKWebView 遮擋偵測（私有 SPI，只在測試包），被蓋住也拍得到卡片介面 |
| long-prompt-scene-hint | 4d78354、4b69c6e、357ba76、e245985、48e346f | 昨晚另一串做到只剩 grok；今晚補 grok 實測（容量取 CLI 本機壓縮點）並抓到、修好「`--verbatim` 讓 grok 共線每輪丟線」的整合 bug |

新立案（未排程）：grok-catalog-parse 後已做完；api-request-header-timeout（API 送出後到回應頭前無逾時）；model-version-follow（CLI 手選型號提醒換新版＋OpenRouter 預填不寫死）。

## 擱置待作者決定（分支已開、交接在分支上）

1. **claude-1h-cache**（分支 `claude-1h-cache`，證據 `.ai/plans/claude-1h-cache.md`）：1 小時快取不是 CLI 固定行為——訂閱 OAuth、沒用到超額、請求來源在 allowlist 內才自動 1h；API key、相容端點、用到超額都是 5m；環境變數 `CLAUDE_CODE_PROMPT_CACHE_TTL`、`FORCE_PROMPT_CACHING_5M` 與 settings `promptCacheTtl` 都能改，`--safe-mode` 擋不住玩家自己的設定。
   - A：app 釘死 1h（帶 `CLAUDE_CODE_PROMPT_CACHE_TTL=1h`），原裁決三件照做。注意：玩家用到超額時，1h 寫入是 2 倍價，花的是玩家的錢。
   - B：每輪照實際寫入分項判斷過期與係數；保溫與離開提醒都要保留。
   - C：app 釘死 5m，維持現有保溫架構（等於推翻原裁決）。
   - 子代理傾向 A＋帳本記實際時效。
2. **image-save-strict-validate**（分支 `image-save-strict-validate`，盤點在 `.ai/plans/image-save-strict-validate.md` 第四節）：八項待決定，含 AI 回 APNG／超大圖／壞圖怎麼處理、角色卡 PNG 匯入嚴驗不過時整張拒收或照匯換乾淨圖、儲存順序。TestCards 21 張真卡嚴驗全過。

## 要作者決定要不要立案

- **grok 伺服器端工具**：即使帶 `--disable-web-search`，grok 仍會在角色回合呼叫伺服器端工具（`backend_tool_call`），照規則丟線重開、該輪輸入翻倍；工具參數曾帶到私設。建議立案查能否從根本關掉。
- **角色台詞前綴**：約三分之一角色台詞被模型自己加「狐狸：」開頭，存進逐字稿、畫面看得到；claude／API 共線同格式，跨傳輸既有行為。建議立案剝除。

## 照模型判斷先做、未經作者裁決〔模型判斷·未裁決〕

- grok 共線：舊的 `chars:<模型>:<角色id>` 線留在 lanes.json 不清理；grok-home 的 `prompt_history.jsonl` 留私設原文（CLI 輸入歷史，實測不進模型上下文）不處理。
- grok 換 session 自救不做（usage-cache-audit 計畫「四」有數據與重評條件）。

## 雜項

- 兩個本地殘留分支 `worktree-agent-a6dcd3607d32fbf75`、`worktree-agent-a6eabbad02b46d707`（昨晚那串建工作樹留下，工作樹已移除）未刪。
- 測試 root `scratchpad/ttroot` 有 grok 登入（新對話的 scratchpad 路徑不同，要重登或另建）。
