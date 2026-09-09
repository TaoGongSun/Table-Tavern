# 免費玩家零門檻開始：OpenRouter 一鍵連接 → 推薦／限時免費模型

Status: in-progress

## Scope
目前只施工第 1 階段「OpenRouter 一鍵連接與免 key」。CLI、高中低 UI、provider-specific panel、推薦模型與限時免費模型全部排除。

## Progress
- 2026-09-09 第一自然工作段完成 Rust 後端 OAuth 核心：`src-tauri/src/openrouter_oauth.rs`。
- 流程依開工當下 OpenRouter 官方 OAuth PKCE 規格：`/auth` + `callback_url` + S256 `code_challenge`，回 localhost `code` 後 POST `/api/v1/auth/keys` 換使用者 API key。
- callback 綁隨機 localhost port；state 以高熵 nonce 放進 callback URL query 並在本機 callback 驗證，避免把 state 是否另行回傳綁死在 OpenRouter 文件表面參數上。
- OAuth 成功後直接讀最新既有 config，寫入 `api_keys["openrouter"]`；若 `best`／`balanced`／`fast` 三個 API tier 全都沒有非空自訂，才一起填 `openrouter/free`。只要任一 API tier 已自訂，三個都維持原樣。
- 已有非空 OpenRouter key 時 command 直接回既有 config，不強迫重跑 OAuth。
- 錯誤目前先回穩定 machine code（browser／timeout／callback／state／cancelled／network／exchange／save），下一段由前端映射十語系可行動文案。
- 新增單元測試：RFC 7636 S256 向量、authorization URL、state／取消／缺 code、bootstrap 不覆蓋、key 落盤且保留既有 config。

## Validation
- 本回合執行環境無法 DNS 連到 GitHub，因此不能 clone repo 到本機跑 cargo；目前完成的是 connector 上的原始碼施工與靜態檢查。
- 下一段前端接好後統一跑 `cargo test`、`npm test`、`npm run check:i18n`、`npm run build`；若 GitHub CI 可用也同步看 branch commit 狀態。

## Next action
把 `src/views/Onboarding.tsx` 改成「連接 OpenRouter」單一主動作；手動 key 收進次要 fallback，補十語系文案與錯誤碼映射，並讓手動 fallback 在完全未自訂 API tier 時套同一個 `openrouter/free` bootstrap。完成後再做第一階段全量自驗與真 OAuth 實機驗收。
