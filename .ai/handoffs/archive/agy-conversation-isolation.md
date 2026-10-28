# agy 對話隔離（假 HOME）

Status: 已結案（2026-10-28 合併 main）

app 呼叫 agy 改走 `<config_root>/agy-home` 假 HOME，對話不再寫進使用者 `~/.gemini`；macOS 以 `Library/Keychains` symlink 共用登入〔作者裁決 2026-10-28〕。方案、測試通道 symlink 規則與已知限制見 [plans/agy-conversation-isolation.md](../../plans/agy-conversation-isolation.md)；待驗三列（首次登入擋發佈、Windows、agy 升版）排在[實測佇列](../../reference/verification-queue.md)。agy-home 對話清理與上游資料目錄需求另案 agy-home-cleanup。
