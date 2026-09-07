# rust-module-homing — owner 判斷表與拍板事項

掃描方式：對 14 支孤兒各掃全 crate 的 `<name>::` 引用（含 `use crate::{a, b}` 群組寫法），依消費者所在的頂層模組歸戶。

## consumer 掃描結果

| 檔 | 行 | 消費者（引用數） |
|---|---|---|
| `ai_transport.rs` | 427 | commands 7（chat／cli_setup／genesis／image／refactor／scene／settings 各 1） |
| `responses_transport.rs` | 384 | ai_transport 4 |
| `proxy.rs` | 115 | install 2、cli/runner 1 |
| `install.rs` | 909 | commands/cli_setup 8 |
| `usage_log.rs` | 683 | lanes 15、transport 6、commands 5、cli 4、ai_transport 2、responses_transport 2、refactor_session 1 |
| `usage_report.rs` | 687 | commands/settings 2 |
| `lanes.rs` | 1577 | commands 15、ai_transport 12、refactor_session 3、cli 1、usage_log 1、usage_report 1 |
| `session_file.rs` | 440 | lanes 17 |
| `snapshot_patch.rs` | 151 | lanes 1 |
| `refactor_session.rs` | 186 | commands/refactor 2 |
| `inflight.rs` | 282 | commands/refactor 8、cli 6、lanes 3、refactor_session 1 |
| `receipts.rs` | 1176 | commands 12（character 7／refactor 2／scene 1／world 2）、refactor 12 |
| `ejs.rs` | 820 | import/mechanism 1 |
| `translate.rs` | 71 | commands/scene 1 |

## owner 判斷〔模型判斷·未裁決〕

判得準的九支：

| 檔 | → | 依據 |
|---|---|---|
| `responses_transport.rs` | `transport/responses.rs` | 唯一消費者是 ai_transport，與 `transport::stream_chat` 同層的另一條 API 路 |
| `ai_transport.rs` | `transport/dispatch.rs` | 職責＝選 CLI 路或 API 路後派送，是 transport 的上層門面 |
| `usage_log.rs` | `usage/log.rs` | 與 report 同一組（JSONL 寫入 → 彙總），消費者橫跨四個模組，不屬於任一方 |
| `usage_report.rs` | `usage/report.rs` | 同上 |
| `lanes.rs` | `lanes/mod.rs` | 續聊線的生命週期，本身就是 owner |
| `session_file.rs` | `lanes/session_file.rs` | 只有 lanes 用（17 處） |
| `snapshot_patch.rs` | `lanes/snapshot_patch.rs` | 只有 lanes 用 |
| `install.rs` | `cli/install.rs` | 唯一消費者 commands/cli_setup，職責＝CLI 安裝／登入 |
| `ejs.rs` | `import/ejs.rs` | 唯一消費者 import/mechanism |

## 【待拍板】

### 1. `proxy.rs` 落點與立案文件衝突

立案文件寫「明顯該歸位的：`ai_transport.rs`、`responses_transport.rs`、`proxy.rs` 三支進已經存在的 `transport/`」，但掃描結果不支持 proxy 這支：它的職責是把 Windows 系統代理塞進**子程序環境變數**，兩個消費者是 `install.rs` 與 `cli/runner.rs`，`transport/` 底下沒有任何一處用到。

- 選 A → `cli/proxy.rs`：與 install 同進 `cli/`，兩支都是「CLI 子程序怎麼起」。
- 選 B → `transport/proxy.rs`：照立案文件，但 `transport/` 會多一支自己不用的檔。

### 2. `receipts.rs`（1176 行）落點

消費者一半在 `commands/`、一半在 `refactor/`，`import/` 反而不直接用它（匯入是經 commands 呼叫）。語意上它是「匯入收據＋一鍵復原」。

- 選 A → 留根層單檔：它服務兩個獨立 feature，沒有單一 owner。
- 選 B → `import/receipts.rs`：語意歸戶，但 `refactor/` 要跨模組引用 import。

### 3. `inflight.rs`（282 行）落點

全域取消訊號表＋子程序 PID 表，`RunEvent::Exit` 也要用（`lib.rs` 直接呼叫 `inflight::kill_all_children`）。消費者橫跨 commands／cli／lanes／refactor_session。

- 選 A → 留根層單檔：跨切面基礎設施，沒有 owner。
- 選 B → `cli/inflight.rs`：PID 表由 `cli::run_cli` 登記，spawn 端是 cli。

### 4. `translate.rs`（71 行）落點

純函式，把一段開場白包成 system＋user 兩則 `transport::ChatMessage`，唯一消費者 `commands/scene.rs`。

- 選 A → `transport/translate.rs`：與 `transport/assemble.rs` 同類（組 messages）。
- 選 B → 留根層單檔。

### 5. `lanes.rs`／`receipts.rs` 要不要拆

行數其實都卡在測試：`lanes.rs` production 1–729、測試 730–1577；`receipts.rs` production 1–704、測試 705–1176。兩支的 production 都在 700 出頭，責任也單一。

- 選 A（建議）→ 只把 `mod tests` 搬成獨立檔（`lanes/tests.rs`、`receipts` 同理），照 repo 既有慣例（`refactor/tests/`、`transport/client/tests.rs`、`cli/runner/tests.rs`）。純搬檔，不動 API。
- 選 B → 連 production 一起依責任拆（lanes 可切「計畫／執行／保溫」三段）。這是動程式結構，不在 structure-only 範圍內。
- 選 C → 都不動。

### 6. 本案之外：要不要替 Rust 也加機器關卡

`scripts/check-structure.mjs` 只掃 `src/`，Rust 側零關卡、全靠 review。本案搬完之後，根層應該只剩 `lib.rs`／`main.rs`／模組根檔——這條規則機器擋得住。要不要做是獨立的一案，不在本案範圍內。
