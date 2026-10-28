# cli-version-gate — CLI 升版輸出格式變了只在執行期才發現

## Summary
2026-10-28 agy-long-survey-fail 審查時 opus 提出：app 解析各家 CLI（agy／grok／codex／claude）的串流與結束事件，CLI 升版改格式時只會在玩家執行時報錯。目前只有個別旗標有版本門檻（如 `agy_supports_stream_json`），沒有統一的「已知可用版本」檢查。

## Next action
未排程。先盤點各 CLI 現有的版本門檻與解析假設，再決定要不要做啟動時版本檢查或解析失敗時的明確提示。
