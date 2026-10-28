# cli-version-gate — CLI 升版改輸出格式只在執行期才發現

規格（三方共識定稿 1b225e4d 之後只有措辭修正）：[plans/cli-version-gate.md](../plans/cli-version-gate.md)。分包：包 1 解析防線、包 2 更新控制與更新鈕，各一筆 commit、各自送驗收。

## 現況
- 分支 `cli-version-gate`，worktree 在 scratchpad `wt-cli-version-gate`；已 rebase 到含 agy-home-cleanup 的 main。
- **包 1 已完成、驗收修正輪已改完**（等主線複驗）：
  - `CliProtocol`（`cli/types.rs`）拆成 id／name／unrecognized／require_done（四家）／sticky_failed_done（codex）／codex_session_checks（codex）。
  - 收尾判定在 `cli/runner/verdict.rs`（純函式，有優先序單元測試）：收尾錯誤 > SoftError（缺收尾時先看，含 context_overflow）> CliCrashed > CliOutputUnrecognized（只在退出 0）> TurnFailed／CliNoReply。
  - 認不得的行：`cli/stream/known.rs`（頂層判別鍵＋必要子判別鍵，缺欄也算）；一輪內收成集合，成功輪只在行程記憶體去重記 log。
  - 上次正常版本與錯誤訊息：`cli/runner/unrecognized.rs`（`cli-last-ok.json` 在帳本旁，只在最終成功後寫；訊息只放版本號）。新 UiMsg 四種 `cli_output_unrecognized_*`，十語系已補。
  - 夾具：`src-tauri/src/cli/stream/fixtures/<cli>/<版本>-<情境>.{jsonl,args.json}`＋`<版本>.version`；契約測試 `cli/stream/contract_tests.rs`（參數逐字、每行認得、數值與識別碼鎖死表 `EXPECTED`、使用者路徑／名字防線）；四家行為測試 `cli/runner/gate_tests.rs`。
  - 夾具擷取：`cli/stream/capture_tests.rs`（檔頭有指令），用 app 的參數組裝函式真的跑 CLI。**重錄後要更新 contract_tests 的 `EXPECTED` 表**（數值與假 id 都會變）。
- **包 2 未開工。**

## 包 2 的起點
- 照方案「包 2」六點做；要點：`cli/spawn.rs` 加更新擋板與依 CLI 的在途表（佔位由 RAII 守衛持有）、關自動更新的 env 統一在 spawn 注入（agy 的 `AGY_CLI_DISABLE_AUTO_UPDATE=true` 已由 agy-home-cleanup 放進 `agy_envs`）、`CliModelUnavailable`／`CliTooOld`／無法分類錯誤的辨識與按鈕、更新流程（目標版本、逾時殺整組、驗證、四家還原契約、分 CLI 的 pending 標記與啟動復原）、首次安裝裝已測版、`cli/tested_versions.rs` 常數與夾具綁定、一次性提示。
- 模型叫不到的四家樣本已在夾具裡（`*-badmodel.jsonl`，`.args.json` 有 exit 與 stderr 末行）；「不認得旗標」的四家 stderr 字樣寫在方案前提段。
- agy-home-cleanup 的 codex delete 會經過 spawn：擋板立起時它拿到 CliUpdating，只記 warn。啟動復原要排在 `home_cleanup::spawn_startup_sweep` 之前，或讓回收也受擋板管。
- 更新鈕與一次性提示的畫面位置，施工時先截圖給作者看再定。
- 實測佇列已加 27：grok `max_turns_reached` 之後會不會送 `end`。

## 注意事項
- **絕不碰作者真實的 CLI 執行檔**（agy 已被誤升級過一次）：一律用副本＋測試 HOME，帶各家關閉自動更新的設定（claude `DISABLE_AUTOUPDATER=1`、agy `AGY_CLI_DISABLE_AUTO_UPDATE=true`（`1` 無效）、grok `GROK_DISABLE_AUTOUPDATER=1`；codex `exec` 本來不觸發）。動手前後 shasum 比對四支：基準在 scratchpad `cvg-author-sha-before.txt`（`readlink -f` 解析後的路徑）。
- 本機副本與測試家目錄在 scratchpad `cvg-cap/`（bin、codexrel、codexhome、agyhome、root），grok 測試登入在 `grok-auth-root/`；編譯目錄 `cvg-target/`。結案時 `cvg-cap/`、`cvg-target/`、`cvg-agytest/`、`cvg-groktest/`、`cvg-agyhome/`、`cvg-samples/` 一併刪。
- 討論串：Sol `codex exec resume 01a1821c-bd11-77b3-985e-5a1ef02008bd`（本案查證）、驗收串由主線開；Grok `01a1821d-0025-7191-98d4-6260a902a786`。agy 直接在測試 HOME 問。
- `cargo test` 前要先 `npm run build:mvu-bundle`（缺 `src-tauri/gen/mvu-engine.js` 會編不過）；跑 verify 前看 `uptime`。

## 下一步
主線複驗包 1；新代理接手包 2。
