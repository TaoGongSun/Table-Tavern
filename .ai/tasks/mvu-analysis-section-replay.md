# mvu-analysis-section-replay — <Analysis> 段的指令也被執行

## Summary
實測佇列第 23 項（2026-10-28，DongeonMaster-safe＋Sonnet CLI）：GM 在 `<Analysis>` 段引用的 `_.set` 也被執行一次，造成重複套用；第二次 insert 回饋成「值沒有變、沒有寫入」，其實第一次已寫入，會誤導模型。證據：scratchpad `vq/23-mechanism-log.jsonl`。

## Next action
未排程；先查上游 MVU 是否也解析 Analysis 段（能模仿酒館就模仿），再定只認更新區塊或去重。
