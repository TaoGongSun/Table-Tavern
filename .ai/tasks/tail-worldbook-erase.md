# 尾段世界書回合後抹掉

Status: todo〔作者裁決 2026-10-11：選 A，回合後抹掉〕

## Summary
GM 續聊線與角色共線的尾段放本輪觸發的世界書（keyword、機率、計時、作者註記、依深度等），回合後不抹，每輪在 session 歷史疊一份：模型看到早已不觸發的舊條目、對話變長、容量量測低估。作者裁決：回合後把尾段世界書從歷史抹掉（沿用 Claude `apply_rewrite`、Grok rewrite），system 不動、維持快取，守住 P2（作者註記與依深度放回合尾段）。Agy 一角一線已全提進 system 並重開，不在此列。背景見 [worldbook-st-trigger-parity 方案](../plans/worldbook-st-trigger-parity.md) 已知差異。

## Next action
未排程，排在 gm-line-system-change-restart 之後。開工先查 GM 線（目前三家都不抹尾段）要接哪套抹寫、共線公開段怎麼併進會抹掉的段落。
