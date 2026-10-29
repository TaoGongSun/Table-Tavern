# claude-bare-default — 查證結果與結論

## 查證（2026-10-29）
- 官方 headless 文件（code.claude.com/docs/en/headless「Start faster with bare mode」）：`--bare` 是腳本與 SDK 的建議模式，「will become the default for `-p` in a future release」；沒給時程、沒提退出方式。bare 不讀 OAuth 與鑰匙圈，Anthropic 認證只認 `ANTHROPIC_API_KEY` 或 `--settings` 的 `apiKeyHelper`。
- cli-reference、env-vars 文件：只有 `--bare` 與等價的 `CLAUDE_CODE_SIMPLE=1`，沒有「不要 bare」的旗標或設定（`CLAUDE_CODE_SIMPLE=0` 未記載為退出方式）。
- CHANGELOG（查到 2.1.312）：沒有「-p 預設改 bare」的條目，也沒有退出旗標。
- 本機 2.1.311（＝已測版）：`--help` 無退出旗標；`claude -p --no-bare` 回 `unknown option '--no-bare'`。

## 結論
現行版本沒有可明帶的退出旗標，**不改程式**。翻轉那版若附退出旗標，在升已測版時一併加到 `cli/request.rs` 的 claude 參數並更新 `.args.json` 夾具；不需要改登入方式。

## 何時會碰到（已知限制）
已測版固定 2.1.311、`DISABLE_AUTOUPDATER=1`，玩家照常用不會碰到。會碰到的路徑：
1. **app 自己的「裝最新版」退路**（cli-version-gate：已測版下載不到、或伺服器要求更新版本時）可能裝到 bare 預設的新版。
2. 玩家自己在終端機升版，或原本就裝了更新的版本（不設版本上限）。
3. 開發者升已測版：擷取腳本 `stream/capture_tests.rs` 用同一套 `request.rs` 參數、作者自己的訂閱登入跑真 CLI，bare 預設版會在擷取時就登入失敗，升常數前一定會發現。

症狀：官方文件寫執行中的失敗（如缺認證）印成 stdout 的 result、不是啟動時拒絕，app 會收到一輪「正常結束」但內容是登入錯誤。本案不改錯誤分類。
