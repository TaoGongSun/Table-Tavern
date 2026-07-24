# Task
Task-ID: cli-install-windows
Title: CLI 一鍵安裝 Windows 支援（PowerShell 分支）
Status: in_progress
Created: 2026-07-24T18:00:00+08:00
Updated: 2026-07-24T18:00:00+08:00

## Summary
2026-07-24 朋友實測 Windows 11 版（release-2 未簽章包），按 CLI 一鍵安裝「沒反應、沒跳終端機」。根因已確診：`install_cli`（src-tauri/src/lib.rs:107-130）是純 macOS 實作——寫出 bash `.command` 腳本後用 `open -a Terminal` 開視窗，`open` 在 Windows 不存在，spawn 當場失敗；前端有 setMessage 顯示錯誤（src/App.tsx:252）但太不顯眼，體感即「沒反應」。就算修掉開視窗這步，bash 腳本在 Windows 也跑不了。

修法：`install_cli` 加 Windows 分支——產 PowerShell 腳本（各 provider 換成 Windows 官方安裝指令），用 `cmd /C start powershell -NoExit -File ...` 開視窗執行；輪詢探針邏輯比照現有流程改寫成 PowerShell。注意 PowerShell 裝完常需新視窗才吃得到 PATH，腳本內要自行補 PATH 或用完整路徑探針。

四家 provider 的 Windows 官方安裝指令查證已外包（general-purpose haiku subagent，2026-07-24 派出），回報後填入下表再實作：

| 供應商 | Windows 安裝 | PATH 行為 | 登入 | 狀態 |
|---|---|---|---|---|
| claude | 待查證（疑 `irm https://claude.ai/install.ps1 \| iex`） | 待查證 | 待查證 | 查證中 |
| codex | 待查證 | 待查證 | 待查證 | 查證中 |
| agy | 待查證（Windows 支援與否未知） | 待查證 | 待查證 | 查證中 |
| grok | 待查證 | 待查證 | 待查證 | 查證中 |

## Next action
- 等查證 subagent 回報 → 主線驗證來源連結 → 填上表 → 實作 Windows 分支（含 cargo test 對應測試）→ 走 release-2 CI 重打包給朋友複測

## Constraints
承 cli-install-all-providers：安裝過程必須可見（開 PowerShell 視窗）；app 不碰帳密／token；只用官方安裝指令，不走第三方；查證回報屬二手摘要，實作前主線須親讀關鍵來源。若某家官方不支援 Windows，按鈕在 Windows 對該家顯示「請手動安裝」引導而非硬跑。另外前端錯誤顯示太不顯眼的問題（invoke 失敗只有一行小字）順帶評估要不要改成明顯的錯誤提示。
