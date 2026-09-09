# 免費玩家零門檻開始：OpenRouter 一鍵連接 → 推薦／限時免費模型

Status: in-progress

## Scope
目前只施工第 1 階段「OpenRouter 一鍵連接與免 key」。CLI、高中低 UI、provider-specific panel、推薦模型與限時免費模型全部排除。

## Progress
- 2026-09-09 第一自然工作段完成 Rust OAuth coordinator：`src-tauri/src/openrouter_oauth.rs`。
- 流程依開工當下 OpenRouter 官方 OAuth PKCE 規格：`/auth` + `callback_url` + S256 `code_challenge`，回 localhost `code` 後 POST `/api/v1/auth/keys` 換使用者 API key。
- PKCE verifier/challenge 由 onboarding 使用平台 Web Crypto 產生；Rust command 先檢查 RFC 長度／字元形狀，避免錯誤參數直接開瀏覽器。後端不需新增 hash/base64 direct dependency，`Cargo.lock` 不必更新；只為 localhost listener 啟用既有 tokio 的 `net` feature。
- callback 綁隨機 localhost port；state 以高熵 nonce 放進 callback URL query 並在本機 callback 驗證；取消、逾時、callback/state、network、exchange、save 都回穩定 machine code。
- OAuth 成功後直接讀最新既有 config，寫入 `api_keys["openrouter"]`；若 `best`／`balanced`／`fast` 三個 API tier 全都沒有非空自訂，才一起填 `openrouter/free`。只要任一 API tier 已自訂，三個都維持原樣。
- 新增 `save_openrouter_key` 給手動 fallback 共用同一個保存／bootstrap 邏輯；已有非空 OpenRouter key 時 OAuth command 直接回既有 config，不強迫重跑 OAuth。
- 2026-09-09 第二自然工作段完成 onboarding UI：主路徑只剩「連接 OpenRouter」一顆主按鈕；不再要求註冊→儲值→建 key→貼 key，也不要求先選模型。OAuth 成功後直接採用後端回傳的 `AppConfig`，畫面自然消失並回到可對話狀態。
- 手動 key 收進 `<details>` 次要入口；前端不再自行組 `api_keys`／`tier_models`，避免跟 OAuth bootstrap 分岔。
- 新增 `src/i18n/features/openrouter-onboarding.ts` 十語系補充字典；`src/i18n/index.ts` 把補充 key 納入既有 `t()`，所以 Onboarding 仍遵守全 app 的「使用者可見文字一律經 `t()`」契約。放在 `i18n/features/` 是為了避免 `check-i18n.mjs` 將它誤認成第 11 個語系檔。
- 前端新增 Web Crypto PKCE 與錯誤碼映射測試：RFC 7636 S256 向量、產生值形狀、十語系補充文案完整性、所有 OAuth 錯誤碼映射、全域 `t()` 路由。
- 已複核既有聊天失敗降級：API HTTP 402／429 會落 `errQuotaApi`，現有文案已提供稍後再試／看供應商額度／換來源；一般 API request/upstream/call failure 也已有可行動分流，本案沒有另改聊天錯誤框架。
- 全程未改 CLI、高中低 UI、provider-specific panel，也未加入推薦／限時免費模型資料。

## Validation
- 目前 connector 環境無本機 Rust／Node 專案工作區，未宣稱已跑 `cargo test`／`npm test`／build。
- 第二工作段以靜態複核收尾：Tauri invoke 參數沿用專案既有 camelCase → Rust snake_case 慣例；補充 i18n 已移出 `src/i18n/` 根層，避免既有檢查腳本誤掃；新增按鈕所在 `.row` 既有可折行契約不需額外 CSS。
- 使用者將在本地執行完整驗證。

## Next action
1. 本地先跑 `npm run verify`；若拆開，至少 `npm test`、`npm run check:i18n`、`npm run build`、`cargo test`。
2. 用全新 config 真實 OAuth 驗收：完全不碰 key／model id／高中低／付款，授權後直接送第一句並收到回覆。
3. 檢查落盤與相容性：全空 API tier → 三檔 `openrouter/free`；既有任一 API tier 自訂 → 三檔不被改；既有 BYOK key 不被強迫 OAuth；手動 fallback 可用。
4. 第 1 階段實機通過後才進第 2 階段。
