# Task
Task-ID: opening-translation
Title: 開場白翻譯：選擇視窗雙鈕（全部翻譯＋翻譯後貼出），走 fast 檔
Status: in-progress
Created: 2026-08-13T00:30:00.811394+08:00
Updated: 2026-08-13T00:30:00.811394+08:00

## Summary
2026-08-08 討論立案。匯入卡片後玩家第一眼要看懂的是開場白，但它不經 AI 重構——重構三類展開（人物／介面／機制）的提示詞都已要求玩家語言輸出（[refactor_ai.rs:165](../../src-tauri/src/refactor_ai.rs#L165)、295、332），純設定條目與開場白則完全不在輸出裡。翻譯掛在開場白選擇視窗（[App.tsx:4780](../../src/App.tsx#L4780)，兩條匯入路徑共用）而非重構管線：需求時點就在「貼出前」，單則翻譯只是幾秒的小呼叫，綁進分鐘級的重構只會製造匯入等待與撤回重問等新機制。

量化（TestCards 實測）：最壞卡 furry-male-scenarios 29 則備用開場白共 26,375 字元，fast 檔（Haiku 級 $1／$5 每百萬 token）全翻約 $0.1–0.2 美元、序列 2–3 分鐘；一般卡不到一分錢、幾秒。

規格細節（設計要點）見 [plans/opening-translation.md](../plans/opening-translation.md)。

## Next action
- 實作完成、四項自驗全綠（cargo 426／vitest 71／build／i18n，2026-08-08），等實機驗收 T1–T7（見交接檔）後結案

## Constraints
- 翻譯呼叫必須玩家主動觸發，不自動跑。
- 開場白內容永遠當資料、永不執行；翻譯失敗退回原文可貼，不擋流程。
