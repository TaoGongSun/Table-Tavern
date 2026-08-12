# Task
Task-ID: character-presence
Title: 角色在場/退場狀態管理：自動上下場＋在場過濾
Status: todo
Created: 2026-08-13T00:30:00.028889+08:00
Updated: 2026-08-13T00:30:00.028889+08:00

## Summary
2026-08-11 orc-cave 實測後立案（使用者裁決）：狀態欄的「駐留角色」等名冊欄位目前只是 AI 回報的清單，沒有接到任何機制。目標＝角色隨劇情自動上下場（在場才進 context、離場收進隱藏區），省 token 也讓側欄反映真實在場狀態。設計地基已寫在 [CARD-REFACTOR-SPEC.md 包 4](../reference/CARD-REFACTOR-SPEC.md)（system 凍結名冊＋首次在場 append 全文、換幕結算 archived、封存三態、present 欄位優先）。

## Next action
- 2026-08-11 立案；地基見 CARD-REFACTOR-SPEC 包 4，開工前逐點重拍板，排序在 refactor-dispatch 之後

## Constraints
- 手動封存是玩家的決定，永不被 AI 自動拉回（包 4 拍板）。
- 幕中不動 system（快取紅線），增減一律走歷史 append 或換幕結算。
