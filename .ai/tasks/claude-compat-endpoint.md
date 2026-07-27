# Task
Task-ID: claude-compat-endpoint
Title: Claude CLI 接 Anthropic 相容端點（DeepSeek／GLM／Kimi）
Status: in_progress
Created: 2026-07-26T00:00:00+08:00
Updated: 2026-07-28T01:20:00+08:00

## Summary
2026-07-26 與使用者拍板：為少數想用中國廠商 Anthropic 相容 API 的使用者，在 Claude CLI 供應商底下加一組選填欄位（自訂 base URL＋該端點 API key）。機制＝spawn claude 子行程時注入 `ANTHROPIC_BASE_URL`（key 非空再加 `ANTHROPIC_AUTH_TOKEN`）；base URL 留空＝行為與現在完全相同。UI 收在設定頁、僅 transport==="claude" 時顯示的 `<details>` 進階摺疊區（預設收合），附「送往第三方、以該家 API key 計費、Claude 訂閱不參與」說明。持久化沿用既有 `api_keys["claude_compat"]`＋`preferences["claude_base_url"]`，免改 schema。實作已完成並通過 cargo test（77 綠）＋npm build。

## Next action
- 暫掛（2026-07-28）：本機沒有 DeepSeek／GLM／Kimi 任一訂閱，無從實測，已從 In progress 移回 Todo
- 有相容端點的訂閱或協力者時再驗：設定頁切到 Claude CLI 展開進階區，填端點（如 DeepSeek `https://api.deepseek.com/anthropic`）實聊一輪，通過即結案

## Constraints
app 不碰帳密儲存以外的憑證流（key 只存本機 config、以 env 傳給子行程）；base URL 留空零行為變化；說明文字必須講明計費歸屬；OpenRouter 的 base_url 欄位（API 直連用）與本欄位互不相干。
