# provider-lang-split：桌面版 AI 連線清單依語系分流（方案）

來源：作者「以後簡體中文的 AI 供應商頁面和其他語言分流：矽基流動只在簡體中文顯示」，簡中由矽基流動取代 OpenRouter（原本說藏 Claude，作者更正：矽基流動跟 OpenRouter 才是同一個角色；四個 CLI 都留）〔作者裁決 2026-11-07〕。網頁版早已分流（`web/src/features/provider/provider-choice.ts::providersFor`，簡中只有魔搭與矽基流動、其他語系只有 OpenRouter〔作者裁決 2026-11-03〕）；桌面版設定頁的連線清單（`src/features/settings/TransportChoice.tsx`）六列對每個語系都列：API 直連、中國供應商（矽基流動）、Claude、Codex、agy、Grok。

案別：小案，核心估約 70 行（清單分流 20、入門卡簡中版 50）。不做方案審，做完 Sol 驗收一次；介面只少列，不跑測試通道、靠 DOM 測試〔模型判斷·未裁決〕。

## 1 規則（`src/features/ai-connection/transport-lang.ts`，一處定義）

- `transportRowsFor(lang)`：簡中 → 不列 `api`（OpenRouter 直連）；其他語系 → 不列 `siliconflow`。四個 CLI 列對每個語系都列。
- 已選中的列不藏：簡中已在用 OpenRouter 或自訂端點、或其他語系已選矽基流動的玩家，那列仍列出（換語系不把人困在看不到的選項上）〔模型判斷·未裁決〕。簡中藏掉 API 直連，自訂端點（中轉站）的進階設定跟著看不到，已設好的不受影響。
- `TransportChoice` 用 `currentLang()` 取語系（App 每次 render 前已同步，設定視窗換語言會重繪）。
- 那列的名稱從「中國供應商（矽基流動）」改成直接叫「矽基流動」：既然只有簡中看得到，不必再標「中國」〔作者裁決 2026-11-07〕。`transportCn` 十語：zh-TW「矽基流動」、zh-CN「硅基流动」、其餘「SiliconFlow」。

## 1a 入門卡（`src/features/ai-connection/Onboarding.tsx`）

- 簡中改推矽基流動〔作者裁決 2026-11-07〕：同一張卡、同一個出現條件（`aiReady` 不動），內容換成：標題「连接硅基流动，即可开始游戏」、一句說明、「申請金鑰」鈕（開 `SILICONFLOW_KEY_URL`，沿用 `cnGetKeyBtn`）、金鑰輸入框（沿用 `cnKeyLabel`）、「保存密钥并开始游戏」。存檔走 `updateConfig({ api_keys: { siliconflow }, preferences: { transport: "api", base_url: SILICONFLOW_BASE_URL } })`，跟設定頁選中國供應商後存檔同一個結果；成功 `onSaved`，失敗顯示 `errSave` 那句。沒有 OAuth、沒有「我已經有 key」折疊、沒有免費模型那句。
- 語系判斷與清單同一處：`transport-lang.ts` 匯出 `defaultApiProvider(lang)`（簡中 `"siliconflow"`、其他 `"openrouter"`），`transportRowsFor` 與入門卡都看它。
- 新文案鍵放 `src/i18n/features/openrouter-onboarding.ts` 那份補充字典（`onboardCnTitle`、`onboardCnIntro`、`onboardCnSave`），zh-TW 正典、zh-CN／en 翻譯、其餘英文。

## 2 不動的

- 後端、`transport` 偏好值、i18n 鍵名（只改 `transportCn` 的字串）、網頁版、`aiReady`、說明書（沒有提到供應商清單）。

## 3 驗證

- `TransportChoice` DOM 測試（放 `src/features/settings/TransportChoice.test.tsx`）：zh-CN 列出矽基流動與四個 CLI、沒有 API 直連；zh-TW 有 API 直連、沒有矽基流動；zh-CN 且 API 直連選中（`transport="api"`、非矽基流動）時那列仍列出；zh-TW 且矽基流動選中時那列仍列出。既有 `SettingsForm.siliconflow.test.tsx` 預設語系 zh-TW，點中國供應商那列的測試要先 `setLang("zh-CN")`（或在測試裡以已選中狀態掛載）。
- 入門卡測試（`Onboarding.test.tsx`）：zh-CN 掛載沒有「連接 OpenRouter」鈕、有申請金鑰鈕與輸入框，貼金鑰送出 → `update_config` 收到 `api_keys.siliconflow` 與 `preferences.base_url`＝矽基流動、`transport`＝`api`；zh-TW 掛載仍是 OpenRouter 一鍵連線。
- `npx tsc --noEmit && npm test`、`node scripts/check-i18n.mjs` 綠；結案前主線 `npm run verify`。

## 4 觀察（範圍外，不立案）

- 簡中入門卡存完金鑰後用的檔位模型是矽基流動的預設（`transport::siliconflow::model_options`），沒有像 OpenRouter 那樣的「免費起始設定」一句；真有免費額度要不要標，等實機。

## 5 施工結果

- 核心約 85 行（規則檔 15、連線清單 showRow 10、入門卡簡中分支 60）、測試約 130、i18n 十語（`transportCn` 改名、入門卡三鍵、`apiKeyLooksPastedSf`）。Grok 施工 4 輪（清單分流、改名＋入門卡、規則更正為取代 OpenRouter、審查修正）、Sol 驗收 2 輪（第 1 輪必改 1：簡中入門卡貼錯提示叫玩家去 OpenRouter 建金鑰，改成矽基流動措辭的新鍵；第 2 輪 PASS）。沒拍截圖：只少列與換文案，靠 DOM 測試〔模型判斷·未裁決〕。
- 已知限制：簡中藏掉「API 直連」後，自訂端點（中轉站）的進階設定只有已選中時看得到；簡中入門卡沒有「免費模型」那句（矽基流動免費額度未實測）。
