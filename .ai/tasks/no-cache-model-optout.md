# 零命中的模型不走共線：自動退回單角色組裝

## Summary

共線每輪多送約 1,500 tokens 換取快取命中。對確定沒有快取的模型（實測 `deepseek-v4-pro-0813-free` 27 筆命中率恆 0）這是純增：該模型平均輸入 4,411，多送 1,500 等於 **+34%**，零回收。要一個「連續 N 輪零命中就不走共線」的自動退回。規格見 [.ai/plans/no-cache-model-optout.md](../plans/no-cache-model-optout.md)。

## Progress

- 2026-08-21 立案。相依 api-shared-lane 的包 B（共線組裝器）先落地。

## Next action

等包 B 完成後開新對話設計：N 的值、per-model 狀態存哪、重置條件、要不要讓玩家看見。

## Constraints

- 相依 api-shared-lane 包 B，不能先做。
- 與「單角色桌私設留 system」是不同的分支條件，勿混為一談。
