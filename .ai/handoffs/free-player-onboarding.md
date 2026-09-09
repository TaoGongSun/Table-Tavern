# 免費玩家零門檻開始：OpenRouter 一鍵連接 → 推薦／限時免費模型

Status: in-progress

## Scope
目前只驗收第 1 階段「OpenRouter 一鍵連接與免 key」。CLI、高中低 UI、provider-specific panel、推薦模型與限時免費模型全部排除。

## Progress
- 2026-09-09 第一自然工作段完成 Rust OAuth coordinator：`src-tauri/src/openrouter_oauth.rs`。
- 流程依開工當下 OpenRouter 官方 OAuth PKCE 規格：`/auth` + `callback_url` + S256 `code_challenge`，回 localhost `code` 後 POST `/api/v1/auth/keys` 換使用者 API key。
- PKCE verifier/challenge 由 onboarding 使用平台 Web Crypto 產生；Rust command 先檢查 RFC 長度／字元形狀，避免錯誤參數直接開瀏覽器。後端不需新增 hash/base64 direct dependency，`Cargo.lock` 不必更新；只為 localhost listener 啟用既有 tokio 的 `net` feature。
- callback 綁隨機 localhost port；state 以高熵 nonce 放進 callback URL query 並在本機 callback 驗證；取消、逾時、callback/state、network、exchange、save 都回穩定 machine code。
- OAuth 成功後直接讀最新既有 config，寫入 `api_keys["openrouter"]`；若 `best`／`balanced`／`fast` 三個 API tier 全都沒有非空自訂，才一起填 `openrouter/free`。只要任一 API tier 已自訂，三個都維持原樣。
- 新增 `save_openrouter_key` 給手動 fallback 共用同一個保存／bootstrap 邏輯；已有非空 OpenRouter key 時 OAuth command 直接回既有 config，不強迫重跑 OAuth。
- 2026-09-09 第二自然工作段完成 onboarding UI：主路徑只剩「連接 OpenRouter」一顆主按鈕；不再要求註冊→儲值→建 key→貼 key，也不要求先選模型。OAuth 成功後直接採用後端回傳的 `AppConfig`，畫面自然消失並回到可對話狀態。
- 手動 key 收進 `<details>` 次要入口；前端不再自行組 `api_keys`／`tier_models`，避免跟 OAuth bootstrap 分岔。
- 新增 `src/i18n/features/openrouter-onboarding.ts` 十語系補充字典；`src/i18n/index.ts` 把補充 key 納入既有 `t()`，所以 Onboarding 仍遵守全 app 的「使用者可見文字一律經 `t()`」契約。
- 前端新增 Web Crypto PKCE 與錯誤碼映射測試：RFC 7636 S256 向量、產生值形狀、十語系補充文案完整性、所有 OAuth 錯誤碼映射、全域 `t()` 路由。
- 已複核既有聊天失敗降級：API HTTP 402／429 會落 `errQuotaApi`，現有文案已提供稍後再試／看供應商額度／換來源；一般 API request/upstream/call failure 也已有可行動分流，本案沒有另改聊天錯誤框架。
- 2026-09-09 第一階段 UI 收尾：`API format` 不再壓在 AI 分頁最上方；一般 `Settings`（連線方式／key／模型等）先顯示，API format 改放頁面後方預設收合的「進階：API 相容設定」。底層仍讀寫同一個 `preferences.api_mode`，Auto／Chat Completions／Responses 行為與舊 config 不變；新增 `src/i18n/features/api-compat.ts` 讓這塊原本硬編碼英文的 UI 走十語系 `t()`。
- 全程未改 CLI、高中低 UI、provider-specific panel，也未加入推薦／限時免費模型資料。

## Validation
- 2026-09-09 使用者曾在 UI 收尾前完整跑 `npm run verify`，七步全綠：structure、cargo fmt、vitest、i18n、build、cargo check、cargo test。
- 首輪驗證曾抓到 rustfmt 差異與 `ulid 3.0.0` 沒有 `Ulid::new()`；已依本機結果修成 rustfmt 格式並將兩處 ULID 產生改為 `Ulid::generate()`，當時最終全綠。
- API format 收合小包只改 `SettingsWindow.tsx`、i18n 入口與 feature-local 文案；diff 複核沒有底層 transport／config migration。因它發生在前次七步全綠之後，仍需重新跑一次 `npm run verify`。
- 尚未宣稱實際 OpenRouter OAuth／免費第一句已通過。

## Next action
1. 重新跑 `npm run verify`，確認 UI/i18n 收尾後仍七步全綠。
2. 用全新 config 真實 OAuth 驗收：完全不碰 key／model id／高中低／付款，授權後直接送第一句並收到回覆。
3. 檢查落盤：全空 API tier → 三檔 `openrouter/free`；既有任一 API tier 自訂 → 三檔不被改；舊 `api_mode` 展開進階區後仍顯示原值。
4. 相容性：既有 BYOK key 不被強迫 OAuth；手動 fallback 可用；CLI 路線與 UI 無差異。
5. 第 1 階段實機通過後才進第 2 階段。
