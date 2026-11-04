> 結案 2026-11-04：包 1–3 進 main（一包一筆），Sol 驗收各包 PASS、常識審查併入。

# room-seat-delegate — 多人房間階段三：AI 代管座位、到場整理、手機房主暫停

Status: done

案別：大案（立案估核心約 350，方案審後 370）。規格、分包、驗收、施工觀察都在 [plans/room-seat-delegate.md](../../plans/room-seat-delegate.md)。

## 成品
- 包 1 伺服器：`seat.delegate`（已送出的先退回草稿、交易尾端結算、抵消、接回登場）、`member.kick` 帶處置、`ctl pause`（房主）、倒數到期改觀望、`invite.current`、收回改字放回接回清單。
- 包 2 執行端：派工 `delegate` 模式進 NPC 池、「玩家暫離」「玩家回來」標記、`ctl pause` 走關房流程、`invite.get` 回現行密語、桌面版遠端暫停通知。
- 包 3 手機與房間元件：代管常駐列與接回、名單頁代管鈕與兩鈕離開確認、到場整理（代管、移除選處置、一鍵代管 `seat.delegate_many`）、邀請新朋友（現行連結＋QR）與換新連結、手機暫停房間、回合列各種說明與重開提醒。

## 部署注意（作者部署時看這段）
房間伺服器必須跟這版桌面版一起部署（先伺服器、再發 app），舊伺服器不認得本案的協定增補：
- 成員指令：`seat.delegate`、`seat.delegate_many`、`invite.current`；`member.kick` 的 `mode`；`ctl` 的 `{ kind: "pause" }`（房主下）。
- 持久事件 `seat.delegated`；快照 `members[].delegated`、`me.delegated`；`RoomState.returning`。
- 派工 `departures[].mode: "delegate"`、`returns`；伺服器→執行端 `invite.get`、執行端→伺服器 `invite.get.result`。
- 倒數到期改成觀望（行為改變，不是新欄位）。
- 舊版桌面 app 收到 `delegate` 模式會當成離席；舊伺服器收到新訊框會回 4400。

## 已知限制
見方案 §8。實機待驗排在[實測佇列](../../reference/verification-queue.md)：其他玩家畫面的「全員代管中，等房主處理」。
