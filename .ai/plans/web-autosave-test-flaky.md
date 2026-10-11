# web 自動存檔測試偶發失敗：根因與修法

## 根因
測試寫法問題，產品沒有競態。

`web/src/features/chat/autosave.test.tsx`「first slot must land before the first dispatch」放行卡住的首格寫入後，只 `settle()`（固定等 20ms）就斷言 `streamChat` 已呼叫 1 次。放行後產品要依序走完：首格寫入（fake-indexeddb 的 put 與 global 寫回）→ `mvu.ready()` → `begin` → `pool.refresh` → 選模 → 送模（`web/src/features/chat/useChat.ts` 的 send 流程，首格段在「新開的桌第一次開口（D27）」那段）。平常約 4ms 走完；負載下超過 20ms，斷言就紅在 `expected "vi.fn()" to be called 1 times, but got 0 times`。

證據（臨時碼都已刪除，未 commit）：
- 複製一份測試，在放行後的首格 put 裡同步忙等 BUSY ms 模擬負載：修前 BUSY=0 3/3 綠、BUSY=30 3/3 紅，錯誤訊息與 verify 那次相同。
- 探針量放行到送模的時間：BUSY=0 約 4ms（settle 回來時 22ms）；BUSY=30 約 34ms，settle 已在 30ms 回來。之後再等就送出了，表示只是測試等得不夠，不是產品卡住。
- 同檔其他「放行後 settle 再斷言」的測試只等存檔寫完、鏈短，同樣忙等 30ms 全綠，不受影響。

玩家影響：無。負載下只是首格寫久一點才送模，首格沒寫成前模型一發都不送（第一段斷言照樣成立）。

## 修法（已實作，只改測試）
放行後改用 `act(() => vi.waitFor(() => expect(streamChat).toHaveBeenCalledTimes(1), { timeout: 2000 }))` 等到送模，不再賭固定 20ms。「放行前一發都沒送」的那段不變。

## 驗證（2026-10-12）
- 修後臨時副本 BUSY=0／30／200 各 3 次，整檔 17/17 綠。
- 反向對照：產品暫改成不等首格就送，修後測試紅在「放行前不該送」；暫改成首格寫成後也不送，修後測試紅在 waitFor 逾時。產品碼已還原。
- `npm run verify`：12 步全綠。
