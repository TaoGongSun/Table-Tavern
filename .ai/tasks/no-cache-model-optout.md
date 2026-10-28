# 零命中的模型不走共線：自動退回單角色組裝

## Summary

共線每輪多送約 1,500 tokens 換取快取命中。對真的沒有快取的模型這是純增，輸入越小佔比越大（`deepseek-v4-pro-0813-free` 平均輸入 4,411，最壞 **+34%**）。要一個「連續 N 輪零命中就不走共線」的自動退回。**前提未成立**：目前還沒有任何模型被證實沒有快取。規格見 [.ai/plans/no-cache-model-optout.md](../plans/no-cache-model-optout.md)。

## Progress

- 2026-08-21 立案。相依 api-shared-lane 的包 B（共線組裝器）已落地。
- 2026-08-22 與 Sol 討論，挖出兩個坑並收斂出方向，全部寫進規格檔：退回目標 `assemble_messages` 已被包 B 刪除（改用 `cards=[本輪角色]` 走同一支組裝器）、退回後不會自然自癒（要主動 probe）。
- 2026-08-22 **立案證據被推翻**：那 27 筆 deepseek 行全部缺 `cache_reporting` 欄，0 分不出是真沒中還是量不到（`usage-diag-non-claude` 修完後一律歸 unknown）。本案目前沒有任何「模型不支援快取」的實測證據。

## Next action

排在 multichar-next-speaker-silent 結案之後〔作者裁決 2026-10-28〕。前提改寫：OpenRouter 背後各家快取行為不同，「證明某個模型沒有快取」靠我們實測沒有意義，退回本來就該是執行時逐模型判定。開工要先確認的是判定的可靠性——帳本能分清「回報為 0」與「沒回報」（`cache_reporting`），只有前者才計入連續零命中。可用免費模型每日額度取「回報為 0」與「沒回報」各一例驗判定。之後拍板規格檔的四項（solo 的 role 分配、要不要讓玩家看見、冷卻週期、與 usage-diag-non-claude 的先後）。

## Constraints

- 相依 api-shared-lane 包 B（已落地，實機驗收未做）。
- 與「單角色桌私設留 system」是不同的分支條件，勿混為一談。
- 只在 api 路徑做自動退回，CLI 三條不做（model 欄可能是「(CLI 預設)」認不出模型）。
