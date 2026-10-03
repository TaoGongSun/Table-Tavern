# card-chat-messages-shim — 卡片介面沙盒墊讀訊息函式

## 現象
NorthHall、NorthHall-structure、RPGImmortal 的「美化状态栏」殼靠 `getCurrentMessageId()`＋`getChatMessages(id)` 讀本樓原文，自己從裡面解析 `<Status_block>`。app 的沙盒沒有這兩個函式，殼拿不到資料，畫面停在「加载中...」。

## 裁決
在沙盒裡墊上讀訊息函式，不藏面板；這次只做讀訊息類，MVU 類另立 [card-mvu-shim](../../tasks/card-mvu-shim.md)〔作者裁決 2026-10-02〕。

## 狀態（已結案）
- 已進 main。規格、生命週期、已知限制與實測結果見 [plans/card-chat-messages-shim.md](../../plans/card-chat-messages-shim.md)；Sol 審查兩輪、驗收通過。
- `npm run verify` 全綠：vitest 504、cargo 731、harness 28。
- GUI 零額度驗收過：NorthHall-structure、RPGImmortal 兩組值都顯示；面板開著時收回／復原會換值；playable: no 的 interface 桌在空桌顯示開場白。
- 排入[實測佇列](../../reference/verification-queue.md)梯 2：面板開著時，GM 新回覆進來會自動換值（低階模型跑一回合）。
- playable: no 桌的後續回合有值，要等 [refactor-statusbar-skeleton](../refactor-statusbar-skeleton.md)。
