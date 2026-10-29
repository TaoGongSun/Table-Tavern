# unix-only-fake-cli-tests — 共用 Rust 假 CLI，讓 lane／runner 代表測試在 Windows 也跑

分支：`unix-only-fake-cli-tests`。案別：中案（核心估 ~260 行；實作 ~300 行程式碼行＝假 CLI 272＋定位 helper 26＋lib.rs 2，不含被改寫的測試本體）。

## 定案做法
- 假 CLI 放 `src-tauri/examples/fake_cli.rs`：`cargo test` 預設會編 examples，`cargo build`／`tauri build` 不編，不進正式包。
- 不用 `[[bin]]`＋`CARGO_BIN_EXE_<name>`：那個環境變數只給整合測試（`tests/`），這些都是 `src/` 內的單元測試；另加 bin 也會被 `tauri build` 編到。
- 測試從自己的執行檔位置找它（`target/<profile>/deps/..` → `examples/fake_cli<EXE_SUFFIX>`），helper 在 `src/test_fake_cli.rs`。
- 角色由 `TT_FAKE_CLI_MODE` 選（不用 FAKE_ 前綴：測試會成批清掉 FAKE_* 旋鈕）：`claude`（取代原 python 假 claude，`FAKE_*` 旋鈕同名）、`agy`（取代原 python 假 agy）、`script`（`TT_FAKE_CLI_SCRIPT` 逐行指令，給 runner 測試）。
- 改寫的測試只換假 CLI 來源與拿掉 `#[cfg(unix)]`；`kill -0` 換成 `cli::update::gate::pid_alive`。

## 已知限制
- 其餘假 CLI 測試（grok、codex、agy_cleanup、lane_lock、自帶 python 腳本的 lane 測試、runner 的 runaway 等）維持 `#[cfg(unix)]`，只在 Unix 跑。
- Windows 上「忽略 SIGTERM」不存在（TerminateProcess 擋不掉），收屍類測試在 Windows 測的是寬限＋強殺＋收屍，不含 SIGTERM 被忽略那一段。
- `cargo test --lib` 不編 examples：沒編過就報找不到執行檔，編過舊版則會默默用到舊的假 CLI。

## 改寫成跨平台的測試（14 個）
- `cli/runner/tests.rs` 7 個：串流＋stdin、stderr 致命錯立即中止、無 result 的 crash、prompt 過長、剝除繼承 ANTHROPIC 環境、長 stdin 與輸出並行、出錯前先收屍。
- `cli/runner/finish_tests.rs` 2 個：成功後卡住寬限強殺收屍、保底與斷流窗逾時。
- lanes 5 個：開線／抹寫／續聊／降級重開（tests.rs）、agy 開線續聊（tests.rs）、GM 續聊失敗重開（gm_restart.rs）、agy 續聊斷流（stall.rs）、claude 續聊輸出失控（runaway.rs）。
- 假 claude／假 agy 換成 Rust 後，原本用它們的其餘 unix 測試也跑 Rust 版（macOS 全綠）。

## 下一步
本機 verify 13 步綠；Windows CI（workflow_dispatch run 40821370927）綠，14 個改寫測試在 log 都有 `... ok`。Sol 驗收 PASS，已結案進 main。

## 觀察
