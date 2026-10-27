# agy-lane-shape-vs-spec — agy 線路形狀與 api-shared-lane 規格不符

## Summary
實測佇列第 14 項（2026-10-28）：agy（gemini-3.8-flash-medium）走續聊（resume），且每個角色各一條 lane，不是 api-shared-lane 規格寫的四路共用一條線；6 通 cached 全 0（含 31 秒內續聊）。

## Next action
未排程；先查是刻意改的（同步 api-shared-lane 文件）還是回歸（修回共線）。
