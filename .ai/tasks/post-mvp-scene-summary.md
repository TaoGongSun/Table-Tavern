# Task
Task-ID: post-mvp-scene-summary
Title: MVP 後：場景切換＋場景摘要
Status: todo
Created: 2026-07-20T02:11:07.442328+08:00
Updated: 2026-07-20T02:11:07.442328+08:00

## Summary
依 NewPlan §8／§8.1：加「換場」動作（current_scene +1，地基已在：transcript 按場景分檔、state 欄位已預留）；換場時 App 發單發呼叫請模型把舊場景壓成摘要，存進本機正典後注入新場景上下文。摘要是 App 端文字，跨供應商通用（2026-07-20 凌晨與使用者對談釐清）。待拍板：摘要由 GM 檔位或 fast 檔生成（建議做成設定項，預設 GM 檔位）。

## Next action
- 等 MVP 驗收後開工；先實作換場鈕＋摘要生成單發呼叫，摘要存 world 目錄並在組裝時注入

## Constraints
不依賴任何供應商 session 或自動壓縮（NewPlan §8.1）；摘要一律存本機正典。
