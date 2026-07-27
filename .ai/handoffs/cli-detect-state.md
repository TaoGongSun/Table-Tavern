# Handoff: cli-detect-state

## Current state
三項全部實作完成、驗證綠，等使用者實測後結案。

## Completed
- 前端三態：`clis` 型別改 `CliInfo[] | null`、初值吃快取（src/App.tsx:315）；render 加 `detecting = clis === null`（src/App.tsx:514），偵測中該列只出「正在偵測…」（src/App.tsx:531），不渲染任何按鈕，杜絕誤按一鍵安裝。
- 後端並行：`probe_cli` 拆出單支探測，各套 5 秒 timeout＋`kill_on_drop(true)`（src-tauri/src/cli.rs:97-124）；`detect_clis` 改 `tokio::join!` 四支同時跑（src-tauri/src/cli.rs:126-135）。用 `tokio::join!` 而非 `join_all`，因 Cargo.toml 的 futures-util 是 `default-features = false`（無 alloc feature）。
- 結果快取：模組級 `cliCache` ＋ `detectClis()` 包裝（src/App.tsx:164-170），設定頁四處與 AI 生圖視窗（src/App.tsx:332、374、426、1593）全改走它，重開設定頁直接顯示上次結果並背景刷新。
- i18n zh／en：`cliDetecting`（src/i18n.ts:65、307）。

## Verification
- `cargo test`：93 passed; 0 failed。`npm run build`：✓ built in 444ms（tsc 無錯）。
- 並行實測（暫時測試，測完已移除）：暖機後 parallel=55.4ms vs serial=152.8ms，約 2.8 倍；四支皆偵測到。冷啟動首測 parallel=463ms 是第一次 spawn 的偏差，加暖機後消失。
- 序列版基準（修改前，本機 `/usr/bin/time`）：claude 0.50s／codex 0.25s／agy 0.34s／grok 0.03s，合計約 1.1s。

## Remaining / Next action
- 使用者實測見 tasks/cli-detect-state.md。重點看：設定頁關掉再開，四列不得再閃出「一鍵安裝」。
- 快取是 per-process 的，App 重啟會重偵測一次（設計如此）；若日後要「安裝完別的 CLI 後立刻反映」，`installCli` 的輪詢已會刷新快取，不需額外處理。

## Constraints
同 tasks 檔。
