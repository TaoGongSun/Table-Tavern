# cli.rs 拆進 cli/

狀態：2026-09-05 拆檔完成，機械／CI／實機三關全過，等合併 main 結案。

## 結果
`src-tauri/src/cli.rs` 2087 行拆成 6 檔＋facade（types／detect／catalog／request／stream／runner），原檔刪除。切線按責任層走，不按 provider 垂直切——四家的串流／usage parser 與共用 runner 契約比參數組裝更緊，垂直切會讓 `CliLine`、`UsageLog`、runner 跨四檔交錯。

facade 只掛 `cli/` 之外真有 caller 的名字。四支 `parse_*_catalog`、`GROK_SAMPLING_OVERLAY`、`CliLine` 這 6 個舊 `pub` 在 `cli/` 外零呼叫端，本體維持 `pub`（模組內有正式呼叫端），不掛 facade。`find_binary` 的 re-export 帶 `#[cfg(target_os = "windows")]`——唯一 crate 內呼叫者 `commands::cli_setup` 的安裝路徑只在 Windows 編譯，不加 cfg 會在其他平台報 unused import。

## 驗收
- `scripts/split-verify/compare.py`：44 個 production item 逐 byte 相同，遺失 0、多出 0、內容變更 0；唯一可見度變動是 `hidden_output` private→`pub(super)`（立案白名單第 2 項，catalog 需要）。
- `test_bodies.py`：26 支測試 body 逐 byte 相同，遺失 0、新增 0。
- macOS `cargo test` 535 passed／0 failed，零編譯警告。
- Windows CI（`ci-windows-verify.yml`，手動觸發，[run 33946808415](https://github.com/TaoGongSun/Table-Tavern/actions/runs/33946808415)）：`detect.rs`／`runner.rs` 有 4 段 Windows-only 程式碼，macOS 編不到，實跑 windows-latest 523 passed／0 failed／4 ignored、零編譯警告，四家 CLI real-install smoke 4 passed。
- 實機（macOS release 包）：四家 CLI 各在既有桌跑完一輪對話（串流、落檔、額度數字都在）→ request／stream／runner 四套 parser 全走過；設定頁四家偵測狀態與版本號都在 → detect；Gemini 模型清單為最新 → catalog；期間換過角色發言 → session args；AI 卡重構跑到一半按取消，事後 app 的子程序表為空 → run_cli 的 PID 登記與收尾。
- 聊天對話沒有取消入口（中止機制只服務重構取消與 app 退出清理），不是拆檔造成的缺口。

規格與白名單見 [plans/cli-split.md](../plans/cli-split.md)，施工前基準見 [baselines/cli-split.md](../baselines/cli-split.md)。同型任務＝[transport-split](archive/transport-split-completed.md)、[mechanism-split](archive/mechanism-split-completed.md)。
