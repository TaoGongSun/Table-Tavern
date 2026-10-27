# mvu-memory-bomb-flaky — mvu_engine 記憶體炸彈測試在負載下偶發失敗

## Summary
`mvu_engine::tests::memory_bombs_fail_the_whole_batch_and_the_runtime_is_rebuilt` 在 mvu-abandoned-threads-flaky 的負載重現（兩份整套 cargo test 並行、模組連跑 20 次）中紅 1 次：負載下 3000ms 內灌不到 64MB 記憶體上限，`Deadline` 先於 `Memory` 到，會讓 verify 假紅。同類：`initialize_fails_on_resource_errors_but_reports_validation_failures`（tests.rs 約 269 行，預期 Memory 實得 Deadline）也在同一負載下紅過。

## Next action
未排程；開工先用 mvu-abandoned-threads-flaky 的負載腳本重現，讓「灌到上限」不受牆鐘期限左右（例如受測期限與準備期限分離、或改用較快觸頂的配置），不靠單純放寬時限。

## 驗收方向
同一負載下連跑 30 次不再偶發失敗，且仍驗得到記憶體上限讓整批失敗並重建 runtime。
