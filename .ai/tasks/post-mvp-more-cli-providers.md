# Task
Task-ID: post-mvp-more-cli-providers
Title: MVP 後：擴充 CLI 訂閱供應商（gemini／grok，依偵測到的 CLI 決定）
Status: todo
Created: 2026-07-20T09:48:42.461107+08:00
Updated: 2026-07-20T09:48:42.461107+08:00

## Summary
2026-07-20 與使用者拍板：CLI 訂閱模式不只 claude／codex，之後要接 gemini CLI（agy）與 grok CLI；要接哪家由 detect_clis 偵測玩家電腦裡裝好的 CLI 決定，偵測到才出現在設定的連線方式選項。每家要補的件：detect_clis 加偵測、headless 單發參數組裝（同 cli.rs 的 claude_args／codex_args 模式）、逐行串流解析器、檔位模型下拉的目錄來源（grok 可參考 Build-Collab-Board scripts/live_server.py 的 `grok models` 指令解析；gemini 待查證有無本機模型快取或列表指令）、風險告知同套流程。tier_models 前綴鍵（如 grok:best）與 tier_override 機制已通用，直接沿用。

## Next action
- 等 MVP 驗收後開工；第一步查證 gemini CLI（agy）與 grok CLI 的 headless 單發介面與模型列表取得方式（grok 先抄 Build-Collab-Board 的做法）

## Constraints
只偵測不代辦（不代裝 CLI）；上下文一律 App 端組裝、不依賴 CLI session（NewPlan §8.1）；模型 id 不寫死，清單來自 CLI 自身（快取或指令）。
