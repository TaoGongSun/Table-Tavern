# stage-card-in-speaker-list — 舞台卡出現在「下一位」點名名單

## Summary
gm-reprints-retired-statusbar 重播時看到舞台卡「灰岩要塞」出現在 GM 的「下一位」點名名單裡。同時記兩個小發現：中文收尾句「要求的狀態欄」與英文 "required state fence" 意思不一致（實測未造成影響）；codex 重印時用繁體鍵，可能在平欄長出重複欄位（未驗證）。

## Next action
規格見 [plans/stage-card-in-speaker-list.md](../plans/stage-card-in-speaker-list.md)。包 1（後端核心）已 commit、verify 12 步綠，待驗收；下一步包 2（側欄開關、發言對象過濾、預設發言對象、十語系，前端 `CharacterMeta` 型別補 `stage`）。

## 驗收方向
舞台卡不出現在可點名的名單；收尾句中英一致。
