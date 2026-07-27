# Task
Task-ID: cli-connected-badge
Title: CLI 已連結狀態記憶：按鈕換「已連結 ✓」＋不重發登入
Status: completed
Created: 2026-07-26T12:00:00+08:00
Updated: 2026-07-28T01:20:00+08:00

## Summary
2026-07-26 使用者實測回報：已登入的 CLI 按「登入／驗證」仍會重發登入請求，希望已連結就藏按鈕。拍板做法：app 以 `preferences["cli_connected:<id>"]` 記住連結狀態——安裝／驗證流程 done→true、error→false，CLI 實聊成功一輪也自動標 true；設定頁該供應商列改顯示「已連結 ✓」＋小顆「重新驗證」（token 過期時的重驗入口），未連結維持原按鈕。Windows 端 agy／grok 的 `pre_probe` 維持 false（未登入時探針會觸發 OAuth 副作用，install.rs 有註解，codex 曾誤翻已退回）；Mac 腳本本就先探針成功即跳過登入，無需改。

2026-07-28 實測發現 Mac 完全不會亮：`install_cli` 在 Mac 只是 `open -a Terminal` 丟腳本，腳本印的「驗證成功」沒有回傳通道，`cli-install-progress` 事件只有 Windows 分支會發，旗標實際上只靠實聊成功那條路。修法：腳本驗證通過後 `touch ~/Documents/TableTavern/.verified-<id>`（Windows 則在 done 階段寫同名檔），新增 `cli_verified` 指令讀它；`install_cli` 開工前先刪舊印記避免讀到上一輪結果；前端安裝輪詢改成「偵測到印記才停」並直接寫入 `cli_connected:<id>`。

2026-07-28 追加：grok 探針從 `grok -p "ok"`（實測 26 秒，真跑一次 grok-4.5 推理，逼近 Windows 端 30 秒探針上限）換成 `grok models`（0.8 秒，只讀本機憑證）。未登入時它是否照樣 exit 0 無從驗證，故 Mac 用 `grep '^You are logged in'`、Windows 新增 `InstallSpec.probe_expect`（探針 stdout 須含該字串才算過）。`grok models` 無 OAuth 副作用，Windows 端 grok 的 `pre_probe` 一併翻成 true——Constraints 那條「已登入仍被要求重登」對 grok 解除，agy 維持原樣（探針無等價指令可換）。Windows 端無機器可自驗。

## Next action
- Mac 實測全通過（2026-07-28）：四家按「登入／驗證」後自動亮「已連結 ✓」、Grok 重新驗證瞬間完成、badge 對齊
- 只剩待 Windows 測試者驗：grok 已登入時直接打勾不彈登入視窗，未登入時仍正常走登入

## Constraints
agy／grok 未登入時禁止無頭執行探針（OAuth 副作用）；已知限制：Windows 上 agy 已登入但從未在 app 內驗證／實聊過者，首次按鈕仍走登入視窗，實聊一次即補上旗標。
