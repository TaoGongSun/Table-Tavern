# Task
Task-ID: cli-install-windows
Title: CLI 一鍵安裝 Windows 支援（PowerShell 分支）
Status: completed
Created: 2026-07-24T18:00:00+08:00
Updated: 2026-08-03T00:00:00+08:00

## Summary
2026-07-24 朋友實測 Windows 11 版（release-2 未簽章包），按 CLI 一鍵安裝「沒反應、沒跳終端機」。根因已確診：`install_cli`（src-tauri/src/lib.rs:107-130）是純 macOS 實作——寫出 bash `.command` 腳本後用 `open -a Terminal` 開視窗，`open` 在 Windows 不存在，spawn 當場失敗；前端有 setMessage 顯示錯誤（src/App.tsx:252）但太不顯眼，體感即「沒反應」。就算修掉開視窗這步，bash 腳本在 Windows 也跑不了。

修法：`install_cli` 加 Windows 分支——產 PowerShell 腳本（各 provider 換成 Windows 官方安裝指令），用 `cmd /C start powershell -NoExit -File ...` 開視窗執行；輪詢探針邏輯比照現有流程改寫成 PowerShell。注意 PowerShell 裝完常需新視窗才吃得到 PATH，腳本內要自行補 PATH 或用完整路徑探針。

四家 Windows 官方安裝指令已查證完畢（2026-07-24：haiku subagent 查官方文件＋主線親抓四個 install.ps1 原文核實，四個網址皆 200）：

| 供應商 | Windows 安裝（PowerShell） | 執行檔位置 | 登入 |
|---|---|---|---|
| claude | `irm https://claude.ai/install.ps1 \| iex` | `%USERPROFILE%\.local\bin`（安裝器自動加 PATH） | 執行 `claude` 走瀏覽器流程（無獨立 login 指令） |
| codex | `irm https://chatgpt.com/codex/install.ps1 \| iex` | `%LOCALAPPDATA%\Programs\OpenAI\Codex\bin`（腳本會寫 User PATH，codex.ps1:743,885） | `codex login` |
| agy | `irm https://antigravity.google/cli/install.ps1 \| iex` | `%LOCALAPPDATA%\agy\bin`（agy.ps1:22；PATH 交給 `agy install` 原生設定） | 首次執行自動走 Google OAuth |
| grok | `irm https://x.ai/cli/install.ps1 \| iex` | `%USERPROFILE%\.grok\bin`（腳本會寫 User PATH，grok.ps1:38,153） | 首次啟動走瀏覽器 OAuth |

實作要點（已拍板）：
- 我們產生的 PowerShell 腳本開頭直接把四個 bin 目錄 prepend 進 `$env:Path`（比照 bash 版 `export PATH` 行），不依賴安裝器改 PATH 或重開視窗。
- claude 在 Windows 無 `claude auth login`，登入流程改為直接執行探針前提示使用者跑 `claude`；探針沿用 `claude -p "ok"` 等 headless 指令。
- grok beta 限 SuperGrok／X Premium+（第三方報導），裝了可能用不了——失敗訊息已有手動安裝引導，暫不特判。

實作歷經兩代：.ps1 腳本版（032759c）→ Rust spec 引擎（73b235e 起，現行）；2026-07-25 依「安裝過程必須可見」約束把四家登入全改回可見終端機視窗並整組刪除 headless 撈 URL 機制。現況與驗證證據見 ../handoffs/cli-install-windows.md。

## Next action
- **結案（2026-08-03）**：Grok 回報者確認連上，Windows 全流程（一鍵安裝 → 登入 → 聊天）通過。系統代理自動下傳這關同時獲證——代理確實傳進了聊天子程序。
- 交付內容與完整現場見 [handoffs/archive/cli-install-windows.md](../handoffs/archive/cli-install-windows.md)。所含修正：PowerShell 安裝分支、四家供應商 Windows 落點探針、「檔案被佔用」白話提示（`7158e65`）、系統代理自動下傳＋連線失敗提示（`9a17562`）、grok 探針換 `grok models`＋`pre_probe`。
- 未做、留待日後拍板：Stage C（每週金絲雀排程、診斷打包按鈕、mac 收編引擎）；安裝階段是否也開可見視窗。已知限制（探針用寫死官方落點路徑、grok 需 SuperGrok／Premium+ 訂閱、CI rust-cache 紅輪不存檔）見封存交接檔。

## Constraints
承 cli-install-all-providers：安裝過程必須可見（開 PowerShell 視窗）；app 不碰帳密／token；只用官方安裝指令，不走第三方；查證回報屬二手摘要，實作前主線須親讀關鍵來源。若某家官方不支援 Windows，按鈕在 Windows 對該家顯示「請手動安裝」引導而非硬跑。另外前端錯誤顯示太不顯眼的問題（invoke 失敗只有一行小字）順帶評估要不要改成明顯的錯誤提示。
