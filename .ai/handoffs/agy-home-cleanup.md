# agy-home-cleanup — app 專用 CLI 家目錄（agy／grok／codex）的定期清理

## 狀態
方案第 5 版三方共識，見 [plans/agy-home-cleanup.md](../plans/agy-home-cleanup.md)。包 1（agy＋grok＋prompt_history＋搜尋索引）已三方驗收通過（Sol、opus-review 複審 PASS），在分支 `agy-home-cleanup` 整理成一筆「包 1」commit，worktree `scratchpad/wt-agy-home-cleanup`。分包大案，整案結案才進 main〔專案規約〕。

## 下一步
1. 包 2（codex）等 codex-lane-resume 合入 main 後開工：在本分支 rebase 到當時的 origin/main，照計畫檔「包 2」施工、送審、驗收，成為第二筆 commit。
2. 整案結案：rebase（先記 base）、root 與 `web/` 跑 `npm ci`、`build:mvu-bundle`、verify，整理成「一包一筆」、確認只含本案檔案，合進 main、push，確認 origin/main 含這兩筆後刪 worktree 與分支，交接檔歸檔、HANDOFF 那條移除。

## Constraints
- 只清 app 自己的家目錄，不碰使用者的 ~/.gemini、~/.grok、~/.codex、~/.claude。
- 不跟隨連結；agy 的 keychain symlink、codex 的 auth.json 不在可刪範圍。
