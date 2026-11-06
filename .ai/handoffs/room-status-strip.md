# room-status-strip — 桌邊狀態列＋房主提醒

Status: 等合併（Sol 驗收 2 輪、截圖主線親看；Grok 寫碼、Sonnet 截圖、主線設計〔作者 2026-11-06 本輪指示〕）
Created: 2026-11-06
案別：中案，核心估約 150 行（純客戶端）。規格見 [plans/room-status-strip.md](../plans/room-status-strip.md)。

## Summary
玩家回饋四項（房主提醒、倒數開關、私下溝通、使用者狀態）的第 1 案：故事頁回合列下方一列座位 chip（頭像、角色名、打字中／已送出／交給 GM／離線／AI 代管），取代回合列的「還在等」文字；房主點 chip 催促或代管；房主收到進房、選角、送出、斷線的 toast＋提示音。手機與桌面嵌入共用。

## 下一步
1. Grok 照方案施工（分支 `room-status-strip`），主線讀 diff、重跑 verify、親看截圖。
2. 結案：squash 進 main，把方案 §7 的案 2、案 3 寫進 BACKLOG 再依序開案。
