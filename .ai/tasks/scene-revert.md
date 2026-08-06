# Task
Task-ID: scene-revert
Title: 換幕可復原：退回前幕＋重新生成摘要
Status: in_progress
Created: 2026-08-06T00:00:00+08:00
Updated: 2026-08-06T00:00:00+08:00

## Summary

2026-08-06 使用者提出兩個缺口：換幕摘要不滿意無法再生成；不小心太早按到換幕，回不到前幕繼續玩。

換幕在資料層本來就是可逆的——`begin_next_scene`（data.rs）只做四件事：叫一次 LLM 產摘要、把摘要當 GM 旁白寫進**新幕的全新檔案** `transcript/{N}.jsonl`、`current_scene` +1、順手存幕名。前幕的 `transcript/{N-1}.jsonl` 一個字都沒被刪。所以復原＝刪掉新幕那個只有一則摘要的檔、場號減一、清掉剛存的幕名。

拍板兩條路（第三條「摘要就地手改」暫不做——app 目前沒有任何訊息就地編輯機制，要新開一套 UI，等這兩條上線後看還缺不缺）：

1. **退回前幕**：純本地檔案操作，不花錢。
2. **重新生成摘要**：前幕 events 還在，重跑 `summary_messages` 覆寫新幕那一則。要花一次全額呼叫（約 $0.131），鈕上標明會再花一次，由玩家自己決定。

守門條件兩者共用：**新幕只有那則摘要、還沒開始玩**才給按（`events.length === 1`）。一有新內容鈕就消失——同「收回上一句」那疊的失效原則（位置已被後話蓋掉）。

## Next action
- 實作完成、四項自驗全綠（cargo 332／build／check:i18n／vitest 22），等使用者實機驗收六項後結案。清單見 `.ai/handoffs/scene-revert.md` 的 Next action。

## Constraints
- 守門只認「這幕只有一則」，不去解析那則是不是摘要——`begin_next_scene` 保證新幕第一則就是摘要。
- 退回不動 `aligned_scene`：退回後 `current_scene` 回到前幕，前幕本來就對齊過，不需重送全樹。
- 快取／lane 不必特別處理：`plan_turn`（lanes.rs:221）看到場號對不上就重開，退回一樣走這條，永遠正確只是少省一次快取。
- 重新生成會花錢，鈕的 title 必須寫明；不做自動重試、不做「不滿意就自動再生」（[dont-spend-players-money]）。
- 生成中（`generating !== null`）兩個鈕都不可按。
