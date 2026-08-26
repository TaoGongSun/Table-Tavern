# lib.rs 拆進 commands/

## Summary
`src-tauri/src/lib.rs` 3841 行、98 個 `#[tauri::command]` 攤在同一檔，是全 repo 最大的程式碼檔。已按領域搬進 `commands/` 底下 10 個檔，另把傳輸分派獨立成與 cli.rs 平級的 `src/ai_transport.rs`，lib.rs 剩 157 行（mod 宣告＋`data_root`／`config_root`＋`run()`）。

純搬家：用 line-range 複製腳本搬，body 逐 byte 未動；lib.rs 那 536 行測試跟著各自的函式走。主風險是 `generate_handler!` 清單漏抄名字——Rust 照樣編譯通過，要等前端 invoke 才 runtime 爆，故驗收以 multiset 比對為主。完整規格見 .ai/plans/lib-split-commands.md。

## Progress
驗收 1–5 全綠（2026-08-26）：98 個 command 的 attribute＋完整簽名 multiset 與 `generate_handler!` 98 項名單拆前後逐字相同；175 個搬移項目 body 逐 byte 相同（無空白正規化）＋2 個共用測試 helper 亦然；`ai_transport.rs` 零 `crate::commands` 引用，commands 之間唯一橫向邊是 `character::load_active_cards`（`pub(super)`）；測試 leaf-name 28 個相同、`cargo test` 530 passed；`RUSTFLAGS="-Dprivate_interfaces" cargo check --all-targets` 綠、零 warning。Sol 審查判定無行為性錯誤，三點意見已採納（測試 helper 窄化成私有、body 比對改成不做空白正規化、文件數字更新）。

## Next action
剩驗收第 6 項：Windows CI（`ci-windows-verify.yml`，本機無 MSVC 工具鏈跑不了，只做過把 cfg 條件從 windows 換成 macos 的半套 flip 驗證）＋真機 invoke smoke（打 release 包開一桌跑一個 GM 回合，踩 chat／scene／state／ai_transport 四個新檔）。兩項都過再 commit。

## Constraints
- 純搬家，任何 body 一個字不改；收斂重複、拆函式、改邏輯都不在本案範圍。
- 只允許 visibility 與模組路徑差異；不跑全 repo `cargo fmt`（HEAD 本來就不是 fmt-clean）。
