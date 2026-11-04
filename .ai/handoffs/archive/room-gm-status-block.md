# Task
Task-ID: room-gm-status-block
Title: 房間故事裡的 GM 狀態圍欄改成一行小字狀態列
Status: done
Created: 2026-11-04
案別：中案（核心約 164 行）

## 要做的事
多人房間＝桌面版搬到線上。GM 回報的狀態圍欄（time／place／present）在房間故事裡露原文：手機照原文露反引號、桌面是程式碼方塊。單人桌面版已經把狀態剝出來給頂部狀態欄（`transport/response.rs` `extract_state_block` → `StateBar.tsx`）。房間兩邊改成一行小字狀態列。方案寫在 [plans/room-gm-status-block.md](../../plans/room-gm-status-block.md)。

## 現在狀態
- 已結案進 main。party test 171、verify 16 步綠；方案審 Sol 1 輪＋常識審查，驗收 Sol 1 輪 PASS，常識審查截圖 1 次＋結案 PASS。已知限制見方案。
