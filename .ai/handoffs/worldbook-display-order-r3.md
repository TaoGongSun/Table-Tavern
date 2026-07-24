# Task handoff
Task-ID: worldbook-display-order-r3
Updated: 2026-07-24T04:41:52.839833+00:00
Status: in-progress

## Goal
讓世界書條目以 displayIndex 控制 UI 顯示順序，新條目置頂並可上下移動，完全不改 order 注入語意。

## Current state
Codex（gpt-5.6-sol）背景實作中（主線 Bash task id：bwjscigi6）；主線待其完成後複驗 cargo test＋npm build＋親讀 diff。

## Completed
交辦規格 scratchpad/task-worldbook-r3.md 已定稿發包；Codex 已定位 data.rs、lib.rs、App.tsx、i18n.ts 既有實作。

## Verification
尚未有可驗證產出；驗收條件見交辦檔（新增置頂、move up/down 對稱、頂端 no-op、舊檔正規化、order 與未知欄位不動）。

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main
- HEAD: 12d9cbde5dcb5ddbf3ae68cec8dca20abde1bc7c
- Dirty: true
- Dirty fingerprint: d91542a780433c62f0413857bbd5f93a0f0beab9db8a5957f44915ed4debad65

## Remaining
Codex 完成→主線複驗→隱藏區交接鏈補結案（handoff complete＋handoff task complete）→commit push→使用者實測 R3。

## Next action
讀 /private/tmp/claude-501/-Users-pachelo-GitHub-Table-Tavern/dbb6d324-9b0b-4b7e-8f34-657fe0286962/tasks/bwjscigi6.output 確認 Codex R3 結果並複驗。

## Constraints
只改 data.rs、lib.rs、App.tsx、i18n.ts（CSS 必要時）；不動 order 語意、transport.rs、匯入匯出；不加依賴。
