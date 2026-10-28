# stage-card-in-speaker-list — 舞台卡出現在「下一位」點名名單

## Summary
gm-reprints-retired-statusbar 重播時看到舞台卡「灰岩要塞」出現在 GM 的「下一位」點名名單裡。同時記兩個小發現：中文收尾句「要求的狀態欄」與英文 "required state fence" 意思不一致（實測未造成影響）；codex 重印時用繁體鍵，可能在平欄長出重複欄位（未驗證）。

## Next action
規格見 [plans/stage-card-in-speaker-list.md](../plans/stage-card-in-speaker-list.md)。包 1（後端核心）驗收 PASS；包 2（前端）已 commit、verify 12 步綠、測試通道走過開關／「讓 X 接話」／換幕，待驗收；下一步包 3（重構盤點判定與套用／撤銷）。

## 驗收方向
舞台卡不出現在可點名的名單；收尾句中英一致。
