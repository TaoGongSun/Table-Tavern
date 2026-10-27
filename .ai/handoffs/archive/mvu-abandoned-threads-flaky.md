# mvu_engine 作廢執行緒名額測試在負載下偶發失敗

已結案進 main。根因是準備步驟與受測卡住操作共用短期限、作廢執行緒靠固定時長 Stall 撐住名額；另修產品競態：worker 先還名額（`Permit`）再送回覆，呼叫端收到回覆立刻再送不再誤判 Busy。全域名額由 `busy`／`Permit` 限制，`stuck` 只是追蹤計數。

- 測試：abandoned、respawns、evicted 用測試限定 `Op::Hold`（開始訊號＋放行把手），受測操作顯式短期限、準備與收尾用預設 5 秒；overlapping 只放寬收尾；回歸測試 `back_to_back_requests_beside_a_held_slot_are_never_busy` 用測試限定 `Op::Linger`，舊順序必紅。
- 驗證：兩份整套 lib 迴圈負載下 mvu_engine 模組 30 跑 0 紅（改前 20 跑 6 紅）；verify 12 步綠；Sol、Grok 審查與驗收 DONE。
- 不在本案：memory_bombs、initialize_fails_on_resource_errors 的 Deadline 搶在 Memory 前（mvu-memory-bomb-flaky）。
