# mvu-abandoned-threads-flaky — mvu_engine 作廢執行緒名額測試在負載下偶發失敗

## Summary
`mvu_engine::tests::abandoned_threads_are_capped_and_counted_until_they_exit` 在 2026-10-27 兩次 `npm run verify` 中各失敗一次（flat-field-reporting、refactor-player-card-user-macro 施工時，當時有其他 worktree 的 verify／測試通道同時在跑），單獨重跑與整套重跑都綠。屬計時相關的偶發紅，會讓 verify 假紅、浪費重跑。

## 已知線索
- 該測試驗 QuickJS 宿主的期限中斷與「執行中＋作廢未結束 2」全域名額（mvu-zod-semantics P3）。
- 先例：已結案的 mvu-shim-flaky-test。

## Next action
未排程；開工先在高負載下重現（例如同時跑兩份 cargo test），找出依賴牆鐘時間的斷言，改成事件／通道同步而非放寬時限。

## 驗收方向
高負載下連跑多次不再偶發失敗，且仍驗得到名額上限。
