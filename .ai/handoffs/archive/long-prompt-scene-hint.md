> 結案 2026-10-07：已進 main，Sol、Grok 驗收通過。

# 桌子太長撞到指令長度上限：先修 Windows、上限前提醒換幕

Status: done

## Summary
公開前必做〔作者裁決 2026-10-04〕。規格、實測數據、拍板與驗收在 [plans/long-prompt-scene-hint.md](../../plans/long-prompt-scene-hint.md)。

- 範圍 1、2：三家 CLI 的 system／正文改走暫存檔與 stdin（claude `--system-prompt-file`、grok agent profile＋`--prompt-file`＋`--verbatim`、agy stdin），不再撞 macOS／Windows 命令列上限。
- 範圍 3：換幕容量提醒與鎖（`src-tauri/src/scene_budget/`），把換幕那次呼叫的長度算進去；送不下就分段摘要，可以停止。各後端上限見 plans §3.4：grok 取 CLI 本機壓縮點（`models_cache.json` 的 context_window×門檻%），不是伺服器的 500k。
- 範圍 4：「這一幕太長」錯誤 `AI_CONTEXT_TOO_LONG`，聊天錯誤列附換幕鈕；claude／codex／API／grok 都只認結構化失敗。
- 2026-10-06 拍板：CJK 估計係數維持 1.45〔作者裁決 2026-10-06〕；G_reply 按每次動作（`action_id`）切段〔作者裁決 2026-10-06〕。

## 未涵蓋
- grok 走真 app 的長桌提醒與鎖（只直接打 CLI 驗了上限與錯誤）。
- 輸入介於 grok 壓縮點與 500k 之間的單發，壓縮後送出的內容會不會被改，沒有實測。
- `read_grok_windows` 讀真檔沒有在真 app 跑過。
- Windows 三家真 CLI 讀暫存檔：排在[實測佇列](../../reference/verification-queue.md)梯 3。
- 既有：clippy（test-harness feature）有 4 個參數過多警告（`lanes/mod.rs`、`receipts.rs`、`updater/rollback_point.rs` 兩處），verify 不擋。
