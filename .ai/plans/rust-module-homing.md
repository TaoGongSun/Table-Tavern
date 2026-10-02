# rust-module-homing — 落點定案

判準：下次找這段程式碼時第一個會去翻哪裡。有單一 owner 才搬；跨切面或自成一個功能的單檔留在根層（STRUCTURE.md：Rust 新 module 先用單檔）。〔作者裁決 2026-10-02：落點由模型依讀取方便決定〕

## 搬遷表（2026-10-02 重掃）

| 原檔 | → | 依據（消費者） |
|---|---|---|
| `ai_transport.rs` | `transport/dispatch.rs` | commands 7 處；選 CLI 路或 API 路後派送，是 transport 門面 |
| `responses_transport.rs` | `transport/responses.rs` | 只有 ai_transport 用 |
| `translate.rs` | `transport/translate.rs` | 只有 commands/scene 用；組 messages，與 `transport/assemble` 同類 |
| `usage_log.rs` | `usage/log.rs` | transport／cli／commands／lanes／smart_free 都寫，不屬任一方 |
| `usage_report.rs` | `usage/report.rs` | 只有 commands/settings 用，讀的就是 log 的檔 |
| `lanes.rs` | `lanes/mod.rs`（測試拆到 `lanes/tests.rs`） | 2169 行，其中測試 1276 行 |
| `session_file.rs` | `lanes/session_file.rs` | 只有 lanes 用 |
| `snapshot_patch.rs` | `lanes/snapshot_patch.rs` | 只有 lanes 用 |
| `refactor_session.rs` | `refactor_ai/session.rs` | 只有 commands/refactor 用；重構兩段判官的 session |
| `install.rs` | `cli/install.rs` | 只有 commands/cli_setup 用；CLI 安裝／登入 |
| `proxy.rs` | `cli/proxy.rs` | install 與 cli/runner 用；子程序環境變數 |
| `ejs.rs` | `import/ejs.rs` | 只有 import/mechanism 用 |
| `receipts.rs` | 留根層，測試拆到 `receipts/tests.rs` | 1182 行；commands 與 refactor 兩邊都用，無單一 owner |

留根層不動：`inflight.rs`（lib.rs 退出時直接呼叫、橫跨 cli／lanes／commands）、`ui_msg.rs`（全 crate 共用）、`openrouter_oauth.rs`（自成功能、lib.rs 直接註冊其 command）。

不另立 Rust 結構關卡：根層單檔本來就合法，機器規則寫不出「該不該有 owner」。〔模型判斷·未裁決〕

## 做法

- structure-only：不改邏輯與對外行為，body 內只允許模組路徑替換；可見度只補到搬家後能編譯所需（跨 domain 用的子模組宣告 `pub(crate) mod`）。
- 引用一律改成新路徑，不用 `pub use` 在舊位置留轉接。`usage::log` 以 `as usage_log` 引入，避免遮蔽 `log` crate。
- 拆出的測試檔：父模組留 `#[cfg(test)] mod tests;`（lanes 的 clippy allow 跟著留），新檔不再包一層 `mod tests`。
- import 重算用 python；搬完 `cargo test`。
- 順手：刪 `_to_delete/`（只剩 `.DS_Store`）與根目錄 `.DS_Store`；更新 `docs/ARCHITECTURE.md` 與 Rust 註解、`.ai/` 現行文件裡的舊檔名。

## 驗收

`npm run verify` 全綠；搬前後 `cargo test -- --list` 清單套用模組前綴映射後一致（搬前 705 passed、1 ignored）。
