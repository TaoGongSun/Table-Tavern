# rust-module-homing — Rust 根層孤兒模組歸位

Status: in-progress（開工日 2026-09-07）
規範依據：[docs/STRUCTURE.md](../../docs/STRUCTURE.md) Rust 段；前案規則見 [source-structure](../plans/source-structure.md)。

## 現況

`src-tauri/src/` 根層 19 支 `.rs`。扣掉 `lib.rs`／`main.rs`，以及 `evaluator.rs`／`genesis.rs`／`refactor_assemble.rs`（Rust 2018 模組根寫法，不算問題）＝**14 支孤兒**（立案文件寫 13，漏了 `refactor_session.rs`）。

已完成 consumer 掃描，owner 判斷表見 [.ai/plans/rust-module-homing.md](../plans/rust-module-homing.md)。

## 約束（立案時定）

- structure-only：不改 API、不改 visibility、不改行為、不重寫函式 body。
- 不套 `domain/`／`services/`／`infra/` 抽象層；不建只有 `mod.rs` 的空殼。
- 每段搬完跑 `cd src-tauri && cargo test`，一段一個 commit。
- import 重算用 python，不用 sed。

## 順手項目

- 刪 `_to_delete/`（只剩 `.DS_Store`）、刪根目錄 `.DS_Store`。
- `NewPlan.md` **留**：README ×2、`docs/ARCHITECTURE.md`、`docs/archive/KICKOFF.md`、多支 `.rs` 註解與 `.ai/tasks/*` 都在引用，是現行參考文件（已標示部分內容被 `.ai/plans/` 取代）。
- 修 27 條壞連結：`.ai/` 現行文件指向 `src-tauri/src/{transport,cli,data,refactor,refactor_ai}.rs` 的連結，目標早已變成同名資料夾；另 `.ai/tasks/chars-lane-rewrite-drop.md` 的 `lanes.rs:583` 要改成 GitHub 語法 `#L583`。搬檔後一次重算。

## 下一步

等使用者拍板三件事（見 plans 檔的【待拍板】段），拍完才開始搬。
