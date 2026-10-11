# WorldEditor 重構重設測試偶發失敗：根因與修法

## 根因
測試寫法問題，產品沒有競態。

`src/features/worldbook/WorldEditor.refactor-apply.test.tsx:282`「a clean existing draft is closed before the reset is sent and never written」用假後端 `refactor_reset_to_source`（:286）先 `await setTimeout(0)` 再看 DOM 上的表單（:287–288），用這個結果判斷送出清回時草稿是否已關。

流程：`resetForRerun`（`src/features/refactor/useRefactorWorkflow.ts:244`）先 `await flushDraft()`。`flushDraft` 就是 `flushAndCloseExistingDraft`（`src/features/worldbook/useWorldbookEditor.ts:465`），它用 `updateDraft` 同步改 `draftRef` 並排一次 `setDraftState`，接著才送 `refactor_reset_to_source`（:246）。也就是說，關草稿的「真相」在送出前就同步完成，但 DOM 要等 React 下一次 render。

這次 render 排在 React act 佇列，由 act 用 Node `timers.setImmediate` 沖掉（react 19.2.7 `enqueueTask`）。測試裡的 `setTimeout(0)` 是 1ms 計時器，與這個 setImmediate 賽跑：事件迴圈進到 timers 階段時，計時器若已到期（負載下常見）就先觸發，這時表單還在畫面上，:296 `formOpenAtReset` 就得到 true。

證據（臨時碼都已刪除，未 commit）：
- 複製一份測試，在 handler 排好 `setTimeout` 後同步忙等 N ms，模擬 CPU 負載：N=0 時 3/3 綠，N=5 時 3/3 紅（`expected true to be false`），可確定性重現。
- 在 handler 裡加探針：進入 handler 時表單在、`setTimeout` 之後表單在、再等一個 `setImmediate` 後表單才消失（`atEntry: true, afterTimeout: true, afterImmediate: false`）。可見送出當下 DOM 本來就還沒關，綠燈只是靠計時器剛好輸給 setImmediate。
- repo 裡其他測試的 `setTimeout(0)` 都包在 `act()` 裡（act 結束時會沖掉佇列），沒有這個問題；裸用在假後端裡、又直接讀 DOM 的只有這一處。

玩家影響：無。自動存、離開、切條目都讀 `draftRef`，送出前就已是關閉狀態；畫面晚一個 render 才收起表單，而清回走 IPC，比一次 render 慢得多。

## 修法（已實作，只改測試）
:283–290、:296 改成：假後端在清回「還沒回應」時等表單關掉，用 `vi.waitFor(() => expect(form()).toBeNull(), { timeout: 200 })` 判斷，成功就記 `closedWhileResetPending = true`、失敗吞掉。:296 改為斷言 `closedWhileResetPending` 為 true。:297–298 不變。

這樣斷言的是「清回回應前表單已關」，不再依賴計時器與 React 排程的先後。已知限制：抓不到「送出後、回應前才關」這種寫法（要寫成不 await 的並行碼才會發生）〔模型判斷·未裁決〕。DOM 本來就無法同步反映送出當下的狀態，嚴格的「送出前」只有 `draftRef` 能證明，而那是內部狀態。

## 驗證（2026-10-12）
用臨時副本（未 commit）在 handler 裡同步忙等 `BUSY` ms：
- 修前（舊寫法，忙等放在排好計時器之後）：BUSY=5，3/3 紅（`expected true to be false`）。
- 修後（忙等放在 handler 開頭）：BUSY=0／5／50，整檔 13/13 綠。
- 反向對照：把產品碼暫改成先清回、再 `flushDraft`，修後的測試在 BUSY=0／5 都紅在 `closedWhileResetPending` 斷言，表示新斷言仍抓得到「清回後才關」。
- `npm run verify`：12 步全綠。
