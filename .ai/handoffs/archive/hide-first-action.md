# Handoff: hide-first-action

2026-10-02 結案：已進 main（Sol 審查與驗收共識），實機項排實測佇列梯 1。

拿掉「轉成世界書條目要先隱藏」的前後端前置，桌上的卡只走原轉換確認框；轉換改取桌獨占（回合／換幕中回 WorldBusy），前端確認前後查 `isBusy()`。規格見[計畫](../../plans/hide-first-action.md)。
