# Task
Task-ID: room-desktop-layout
Title: 桌面版房主的房間畫面整理（資訊列一行、錯誤一處、操作鈕放對位置、單層捲動）
Status: in-progress
Created: 2026-11-04
案別：大案（核心估約 440 行，分三包；方案與驗收都送審）

## 要做的事
多人房間＝把桌面版搬到線上，房主在電腦上要跟單人遊玩一樣好用；排在公開 v0.5.0 之前。上方固定區塊（資訊列、回合列、頁籤、房主鈕列、輸入區）太高，故事區只剩最小高度、矮視窗雙層捲動；room-story-area-fix 已止血。作者列的 14 項與補收的幾項，做法、草圖、手機影響、分包見 [plans/room-desktop-layout.md](../plans/room-desktop-layout.md)。

## 現在狀態
- 包 A 已驗收。包 B 已施工並修完 Sol 驗收第 1 輪（換模型照 hold.actions 送 switch_route／switch_model、彈層 QR 160px 選擇器），等 Sol 第 2 輪。verify 16 步綠、party test 166。
- 測試通道做法：本機 Node 殼（party `npm run build`＋`build:node`，PORT=8799）＋`scripts/harness-fake-openrouter.mjs`（第一段成功、第二段 content_filter、之後成功），harness 設定檔 base_url 指假端點 `/api/v1`；視窗用 System Events 調大小（程序名 table-tavern）。

## 下一步
做包 C（方案 §7：第 10、11（到場整理每列收進「…」，用 `party/src/features/controls/PartyMenu.tsx`）、12、13 項與補 A／B／C／E），做完停下等驗收。施工紀錄寫在方案 §10–§12。
