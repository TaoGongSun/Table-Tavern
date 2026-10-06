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

新立案（未排程）：api-request-header-timeout（API 送出後到回應頭前無逾時）；model-version-follow（CLI 手選型號提醒換新版＋OpenRouter 預填不寫死）。

## 早上收尾（2026-10-07）

夜間擱置與待立案的四項，作者醒來決定後都已結案進 main：

| 案 | commit | 作者決定 |
|---|---|---|
| claude-1h-cache | 87621e9 | 續聊線釘 1h、撤保溫與離開提醒、超額只提示一次、單發與重構不釘 |
| image-save-strict-validate | 44cebbd | 八項照建議；GM 圖比照角色卡先救；寫檔 IO 失敗回報錯誤、草稿保留 |
| grok-backend-tools | 6479e19 | 關掉 grok 伺服器端工具，生圖一起關 |
| char-line-prefix | ebcca12 | 角色台詞剝掉模型自加的「名字：」 |

過程中新立案（未排程）：runaway-output-cap（模型持續吐垃圾沒有單輪上限）、claude-resume-tail-cache（claude 續聊對話尾端每輪重寫快取）。

## 照模型判斷先做、未經作者裁決〔模型判斷·未裁決〕

- grok 共線：舊的 `chars:<模型>:<角色id>` 線留在 lanes.json 不清理；grok-home 的 `prompt_history.jsonl` 留私設原文（CLI 輸入歷史，實測不進模型上下文）不處理。
- grok 換 session 自救不做（usage-cache-audit 計畫「四」有數據與重評條件）。

## 雜項

- 測試 root `scratchpad/ttroot` 有 grok 登入（新對話的 scratchpad 路徑不同，要重登或另建）。
