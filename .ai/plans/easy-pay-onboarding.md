# 簡易付費入口：OAuth 一鍵連接 →（條件觸發）App 內儲值

本檔存放 [easy-pay-onboarding](../tasks/easy-pay-onboarding.md) 的規格細節（拍板結論、分包、驗收等），由任務檔的 Summary 連回。

## 第一階段：OAuth 連接（先做）
OpenRouter 官方支援 OAuth PKCE（含 localhost callback）。流程：app 產 code verifier → 開系統瀏覽器到 OpenRouter 授權頁（玩家在該站登入、儲值）→ callback 回本機 → 用授權碼向 `/api/v1/auth/keys` 換 key → 存進現有 key 儲存，依內建推薦清單自動選模型。
工程：Tauri 端 callback 監聽、開瀏覽器與換 key 流程、餘額不足（HTTP 402）顯示「前往儲值」鈕、失敗／取消文案。規模約數天。全程不出現 key／model／endpoint 字眼。
驗收：找沒碰過 API 的玩家實測，不經口頭協助，兩三分鐘內送出第一句話，且說得出「錢是付給誰」。

## 第二階段：App 內儲值（條件觸發，非預設路線）
觸發條件：第一階段實測中多數新手明確卡在「不願註冊外部帳號／在外部網站付款」。沒有此訊號就不做。
開工前提（缺一不動工，先問——被拒就省下整個後端）：
1. 支付商書面核可：誠實申報「允許成熟題材的通用 RP 工具、販售不可轉讓的遊玩用量」，由對方判定 stored value 與內容限制（Paddle AUP 禁 stored value 與成人內容；OpenRouter 條款禁轉售 API 存取——技術上能代發 key 不等於商業授權）。
2. 模型供應書面核可：經 relay 架構取得 OpenRouter 商業同意，或改走模型商（Anthropic／Bedrock／Vertex）商業條款。relay 提高可核准性，不取代核准本身。
工程形狀：小而必須把錢和權限做對的後端——帳號與購買紀錄、付款 webhook 與對帳、key 只存伺服器、客戶端經 relay 發遊戲形狀請求、依地區過濾模型、退款與停權。此步＝從「軟體商」轉「服務營運商」，內容與年齡驗證義務隨之上身。

## 地區阻擋（僅第二階段需要，最低標準三件）
1. 金流端不啟用 WeChat Pay、不提供 CNY 計價。
2. relay 程式內擋來源國家（託管平台附的國家 header ＋三行 if；封鎖清單抄模型供應商官網的不支援地區頁）。
3. 服務條款一句「不向中國大陸及供應商不支援地區提供」。
不需要：證件驗證、VPN 偵測、語言歧視。玩家翻牆繞過＝責任在他。中國版若做＝SFW 獨立模型政策版，無成人 fallback。
