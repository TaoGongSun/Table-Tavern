# Task handoff
Task-ID: worldbook-display-order-r3
Updated: 2026-07-24T04:48:13.429495+00:00
Status: in-progress

## Goal
讓世界書條目以 displayIndex 控制 UI 顯示順序，新條目置頂並可上下移動，完全不改 order 注入語意。

## Current state
實作完成、主線複驗全綠（cargo test 60/60、npm build ✓）；已 commit push，剩使用者實測 UI。

## Completed
read_worldbook 依 displayIndex 排序（缺值排底）；新增條目 displayIndex=0 其餘 +1（含舊檔正規化）；move_worldbook_entry 交換相鄰、邊界 no-op；前端 draft 表單移到清單前、每列 ↑↓ 鈕（首末列 disabled）；i18n 雙語。

## Verification
主線本機 cargo test 60 passed; 0 failed；npm build ✓ built；親讀 data.rs:703（排序）、data.rs:723（置頂插入）、data.rs:772（move）、App.tsx:643（moveEntry）與 ↑↓ disabled 邏輯。order 與 transport.rs 零改動。

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main
- HEAD: 1a84e3269e3c11a1a6af4d0b37db19b37396056c
- Dirty: false
- Dirty fingerprint: 4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945

## Remaining
使用者實測：新增條目出現在最上、↑↓ 移動生效。

## Next action
請使用者重載 dev 實測新增置頂與 ↑↓ 移動，通過後隨 worldbook-st-format 一起結案。

## Constraints
不動 order 語意與注入邏輯；不改匯入匯出。
