# Task
Task-ID: post-mvp-scene-summary
Title: MVP 後：場景切換＋場景摘要
Status: in-progress
Created: 2026-07-20T02:11:07.442328+08:00
Updated: 2026-07-24T01:50:00+08:00

## Summary
依 NewPlan §8／§8.1：加「換場」動作；換場時單發呼叫把舊場景壓成摘要存本機正典。2026-07-24 主線拍板：摘要以 GM 旁白事件（【前情提要】前綴）寫入新場景 transcript 開頭——角色/GM 上下文、匯出、下次換場的鏈式壓縮全部自然沿用，不另做注入管線；摘要檔位固定用 GM 檔位，不做設定項（YAGNI）。

## Next action
- 前後端完成（cargo test 43 綠＋npm build 綠，見 handoffs/post-mvp-scene-summary.md）；剩使用者實測換場鈕即結案

## Constraints
不依賴任何供應商 session 或自動壓縮（NewPlan §8.1）；摘要一律存本機正典。
