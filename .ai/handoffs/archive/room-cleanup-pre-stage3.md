> 結案 2026-11-04：進 main；Sol 驗收 PASS、常識審查 1 輪文案修正。

# Task
Task-ID: room-cleanup-pre-stage3
Title: 座位代理包 3 開工前整理房間程式三條（GM 錯誤分類統一、party/App.tsx 不當函式庫、平台殼路由）
Status: done
案別：小案（核心新增約 98 行，另有約 69 行原樣搬檔）

## 結論
拍板結論、歸類表與文案見 [plans/room-cleanup-pre-stage3.md](../../plans/room-cleanup-pre-stage3.md)。
- GM 錯誤種類只由執行端 `classify` 決定（剝 `AI_CALL_FAILED` 殼認內層碼），面板帶 `lastErrorKind`，前端不再認碼。
- `party/src/App.tsx` 只留入口，房間元件在 `party/src/features/room/RoomBody.tsx`、`useRoomSession.ts`。
- 平台殼路由不抽〔模型判斷·未裁決；主線裁定〕。

## 已知限制
- `AI_HTTP_STATUS_403` 歸金鑰／登入失效；地區或內容被擋時說法會誤導。

## 交給後續
- room-seat-delegate rebase：DesktopRoom「最近錯誤」那行手解，`ROOM_PAUSED_REMOTELY` 在 `LastError` 前特判。
- 重試／換模型鈕搬到回合列、held 時故事區被擠到看不見：進 room-desktop-layout。
