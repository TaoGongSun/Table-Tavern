# Task
Task-ID: rust-module-homing
Title: Rust 根層孤兒模組歸位（順手清 repo 根目錄雜項）
Status: backlog
Created: 2026-09-07T00:00:00+08:00
Updated: 2026-09-07T00:00:00+08:00

## Summary

`src-tauri/src/` 根層有 19 個 `.rs`，其中 `evaluator.rs`／`genesis.rs`／`refactor_assemble.rs` 各有同名資料夾（Rust 2018 的模組根寫法，不算問題），其餘 13 支是真孤兒。

明顯該歸位的：`ai_transport.rs`、`responses_transport.rs`、`proxy.rs` 三支進已經存在的 `transport/`；`usage_log.rs` 與 `usage_report.rs` 是同一組。其餘（`lanes.rs`、`receipts.rs`、`install.rs`、`ejs.rs`、`session_file.rs`、`inflight.rs`、`snapshot_patch.rs`、`translate.rs`）要先看 consumer 才判得出 owner，不得憑檔名決定。

另外 `lanes.rs` 1577 行、`receipts.rs` 1176 行都超過專案的 1000 行軟規則，開工時一併評估要不要拆——拆依責任，不依行號。

順手做掉的 repo 根目錄雜項（十分鐘的事，不值得單獨立案）：刪掉只剩 `.DS_Store` 的 `_to_delete/`、刪根目錄 `.DS_Store`、判斷 `NewPlan.md` 還是不是現行入口（先掃引用再決定去留，不可製造死連結）。

[source-structure](../plans/source-structure.md) 已經替前端訂好長期規則，本案是同一套規則在 Rust 側的落地，規範內容見 `docs/STRUCTURE.md` 的 Rust 段。

順手修 27 條壞連結：`.ai/` 現行文件裡指向 `src-tauri/src/transport.rs`、`cli.rs`、`data.rs`、`refactor.rs`、`refactor_ai.rs` 的 markdown 連結全部失效——那些檔案在更早的 Rust 拆分案裡已經變成同名資料夾，連結沒跟著改。另有 `lanes.rs:583` 用冒號寫行號（GitHub 語法是 `#L583`）。清單用這段掃得出來：

```bash
python3 -c "
import re,pathlib
for f in pathlib.Path('.ai').rglob('*.md'):
    if 'archive' in str(f) or 'history' in str(f): continue
    for m in re.finditer(r'\]\((\.\./[^)#]+)', f.read_text()):
        if not (f.parent/m.group(1)).exists(): print(f, m.group(1))
"
```

## Next action
- 未排程。開工首步＝逐支孤兒檔掃 consumer 列出 owner 判斷表，判不出來的停下來問，不得自行改判成「維持現狀」。

## Constraints
- structure-only：不改 API、不改 visibility、不改行為、不重寫函式 body。
- 不為了跟前端對稱而套 `domain/`／`services/`／`infra/` 這類抽象層。
- 單一責任的模組維持單檔，不預先建立只有 `mod.rs` 的空殼資料夾。
- 每段搬完跑 `cd src-tauri && cargo test`。
