# 生圖下載測試在 verify 中偶發逾時

Status: todo

## Summary
interface-card-panel-v2 驗收期間，主線跑 `npm run verify` 兩次在 cargo test 步驟出現同樣 4 條紅：`generated_image::tests::download_rejects_undeclared_oversize_body`、`download_rejects_declared_oversize_body`、`download_rejects_non_success_status`、`download_sends_no_authorization_header`，都是 `AI_IMAGE_DOWNLOAD_FAILED: timed out after 5s`。第二次機器沒有其他負載仍重現；單獨跑該模組、單獨跑整套 `cargo test --lib`、第三次 verify 都綠。該分支沒碰 generated_image。施工代理那邊的 verify 都綠。

## Next action
未排程。先查本機假伺服器（tests.rs 的 `serve`）在整套並行下為何連不上或回得慢（埠、執行緒、accept 時機、verify 前幾步留下的程序），再決定修測試還是放寬時限。
