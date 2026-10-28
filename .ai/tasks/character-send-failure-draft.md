# character-send-failure-draft — 對角色送出：對白已上桌後才出錯仍彈帶原文的失敗視窗

## Summary
multichar-next-speaker-silent 審查（2026-10-28）發現：對 GM 送出已改成「正文落檔後的任何失敗都不收回玩家句、彈窗不帶原文」，但對角色送出時，角色對白落檔後 `markCliConnected` 或 `refreshWorlds` 拋錯，仍會走帶原文的失敗視窗，誘導玩家重送一句已在桌上的話。既有行為，非本案引入。

## Next action
未排程，等 multichar-next-speaker-silent 結案後比照它的「落檔即記」做法套到角色回合。

## Constraints
- 失敗後玩家句與已落檔的對白都不得遺失或重複。
