> 結案 2026-10-07：已進 main，Sol、Grok 驗收通過。

# grok-backend-tools — grok 角色回合叫出伺服器端工具

立案 2026-10-07（作者同意開工）。

## 問題
grok CLI 1.0.46 即使帶 `--disable-web-search`，角色回合仍會呼叫伺服器端工具（session 紀錄出現 `backend_tool_call`，見過 x_search、send_feedback）。現行規則看到就丟線重開，該輪輸入量翻倍；工具參數曾帶到角色私設（隱私外洩到 xAI 端工具）。

## 目標
從根本讓 app 呼叫的 grok 叫不出伺服器端工具；做不到才退而求其次（偵測後處理），並先回報差距。

## 結果
- `grok_envs` 加 `GROK_BACKEND_SEARCH=0`，`GROK_CONFIG_OVERLAY` 加 `features.backend_tools:false`；聊天與生圖共用〔作者裁決 2026-10-07〕。驗收與範圍外觀察見 [plans/grok-backend-tools.md](../../plans/grok-backend-tools.md)。
