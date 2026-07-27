# Task
Task-ID: cli-install-all-providers
Title: CLI 一鍵安裝擴充到 claude／codex／grok（比照 agy）
Status: completed
Created: 2026-07-24T15:30:00+08:00
Updated: 2026-07-28T01:20:00+08:00

## Summary
2026-07-24 使用者整理 README 時發現：一鍵安裝只有 agy（Gemini CLI）有做，claude／codex／grok 只有偵測，不公平；拍板在測試打包前補齊。做法：沿用 agy 現成框架（`lib.rs` `install_agy_cli`——寫 `.command` 腳本、開可見終端機、官方安裝 script、headless 探針驗證），泛化成 `install_cli(provider)`。三家官方通道已查證（2026-07-24）：

| 供應商 | 安裝 | 登入 | 探針 |
|---|---|---|---|
| claude | `curl -fsSL https://claude.ai/install.sh \| bash`（裝到 `~/.local/bin`） | `claude auth login`（本機 `claude auth --help` 證實） | `claude -p ok` |
| codex | `curl -fsSL https://chatgpt.com/codex/install.sh \| sh` | `codex login` | `codex exec ok` |
| grok | `curl -fsSL https://x.ai/cli/install.sh \| bash`（裝到 `~/.grok/bin`） | `grok login`（本機 `grok --help` 證實） | `grok -p ok` |

登入指令會阻塞到完成才返回，腳本可安裝→登入→短輪詢探針收尾（agy 因 OAuth 不阻塞才需 600s 長輪詢，其餘三家不用）。

## Next action
- 使用者實測一鍵安裝鈕（至少一家非 agy）後結案，接 test-build-cross-platform 重打包

## Constraints
安裝過程必須可見（終端機視窗）；app 不碰帳密／token，OAuth 全在官方頁與 CLI 端；只用官方安裝 script，不走第三方；風險告知照舊前置；探針失敗訊息要引導使用者重開終端機腳本。
