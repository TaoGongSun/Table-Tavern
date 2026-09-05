# cli.rs 拆進 cli/：立案計畫

分支：`cli-split`  
立案基準：`main` / `8dfecfa20b9c3d026752a76c9848312f29c7d5fe`  
原 `src-tauri/src/cli.rs` blob：`2fb6f4ab9808e5b64a1c1c4fd9aaab2ac0e4a212`

本案沿用 `transport/`、`mechanism/`、`refactor_ai/`、`import/`、`refactor/` 已確立的拆檔規則：**純搬家、production body 逐 byte 不動、只允許 module plumbing 所需的 import／visibility／相對路徑調整、`mod.rs` 只當 facade、零呼叫端的 re-export 不掛**。不趁拆檔改 CLI 旗標、不改 timeout、不改錯誤分類、不改串流 parser、不改 prompt cache／usage 計帳行為。

本輪只立案，不修改任何 `.rs`。下一階段施工前必須先完成 immutable baseline、caller inventory、test owner 與 visibility ledger；本文件的切線是依目前實際責任與可見依賴做的施工草案，**以開工前依賴複核結果為準，不能拿行號相鄰直接硬切**。

## 1. 為什麼現在拆

`src-tauri/src/cli.rs` 共 **2087 行**：

- production：1–1176
- 空行：1177
- `#[cfg(test)] mod tests`：1178–2087

production 雖只有約 1176 行，但已同時承擔六種彼此不同的責任：

1. CLI binary 探測與版本偵測；
2. 四家 CLI 的模型目錄讀取／解析；
3. prompt 攤平、tier 對應與 provider args/env 組裝；
4. 四家 streaming JSON parser；
5. 四家 usage／prompt-cache 計量 parser；
6. 子程序生命週期、stdin/stdout/stderr、timeout、inflight PID 與 usage 落檔。

這些責任已有明顯單向邊界；繼續堆新 CLI provider、custom provider、cache／session 行為時，單檔會讓 provider-specific 變更與共用 runner 更容易互相踩到。

## 2. 施工前基準：必做，不得省略

目前 production 頂層 item 初步盤點為 **45 個**：

- `pub`：33
- `pub(crate)`：1（`find_binary`）
- private：10
- `impl`：1（`Drop for ChildPidGuard`）

開工前要用基準 blob 機械產生正式 manifest，記錄每個 top-level item 的：

- 名稱／種類／原 visibility；
- 原始行號區間；
- 原文 hash／byte slice；
- 施工後 owner 檔案；
- 是否允許 visibility 調整。

同時另外建立：

1. **caller inventory**：逐項搜尋 `cli.rs` 外 Rust caller，決定 facade 真正需要保留的 `cli::...` API；
2. **test manifest**：數出全部 `#[test]` leaf，記錄每支測試的原 body 與新 owner；
3. **visibility ledger**：凡 private／`pub(crate)` 因跨 sibling 模組需要放寬，必須事前列白名單；
4. **dependency DAG**：只看 production 真實符號引用，不用行號鄰近推測依賴。

施工完成後用 `scripts/split-verify/` 的既有工具或等價機械檢查，對 immutable baseline 做逐 item／逐 test body 驗證。

## 3. 現有 public API 的外部 caller 情況

目前已確認 `cli.rs` 外有實際 caller 的範圍至少包括：

- `data/config.rs`：`ModelOption`
- `transport/client.rs`：tier/model 選擇、provider args、stream parser、usage parser、`run_cli` 等 CLI 傳輸主路徑
- `lanes.rs`：`CliSession`、Claude／Grok session args 與 runner
- `refactor_session.rs`：CLI session／runner
- `inflight.rs`：`run_cli`、`parse_claude_line`
- `transport/assemble.rs`：`flatten_messages`
- `commands/cli_setup.rs`：CLI 探測／Grok profile 相關功能（含平台條件路徑與 tests）

因此本案**不能**為了模組漂亮改掉既有 `crate::cli::...` 呼叫路徑；真正有 caller 的項目由 `mod.rs` re-export 維持相容。

反過來，現在宣告為 `pub` 但經完整 caller inventory 證明只有 `cli.rs` 自身／同模組 tests 使用的項目，不因「以前是 pub」就自動掛到 facade；是否需要 sibling visibility 由 ledger 決定。

## 4. 草案切線

先以 **6 個 implementation 檔＋純 facade** 為施工草案：

| 檔案 | 責任 | 主要 item |
|---|---|---|
| `types.rs` | CLI 共用資料型別／runner contract | `CliInfo`、`ModelOption`、`CliSession`、`CliLine`、`UsageLog` |
| `detect.rs` | binary 尋找、可執行檢查、版本探測、四家並行偵測 | `candidate_dirs`、`is_executable`、`find_binary`、`hidden_output`、`probe_cli`、`detect_clis` |
| `catalog.rs` | 四家模型目錄解析與 catalog 組裝 | `parse_codex_catalog`、`parse_claude_registry`、`parse_agy_catalog`、`parse_grok_catalog`、`cli_model_catalog` |
| `request.rs` | prompt 攤平、tier mapping、四家 args/env/session args | `flatten_messages`、`tier_override`、`claude_model_for`、`codex_effort_for`、Claude/Codex/Agy/Grok args、Grok env/constants |
| `stream.rs` | 四家串流事件與 usage parser | `parse_*_line`、`parse_*_usage`、`usage_event`、`token_count` |
| `runner.rs` | 子程序生命週期與共用 headless runner | `ChildPidGuard`、`api_error_kind`、`run_cli` |
| `mod.rs` | module 宣告＋有 caller 的既有 API re-export | 不放 implementation |

### 為什麼先不按 provider 拆成 claude.rs／codex.rs／agy.rs／grok.rs

目前四家 provider 的「參數組裝」彼此獨立，但 streaming／usage parser 和共用 runner 的共同契約更強。若直接按 provider 垂直切，`CliLine`、`UsageLog`、catalog 探測與 runner 會跨四檔交錯，容易製造重複 import 與反向依賴。

所以第一優先是依**責任層**切：detect → catalog、request、stream → runner。若開工前 DAG 顯示 provider 垂直切更乾淨，可以調整，但必須把變更理由寫回本計畫；不能只是為了平均行數。

## 5. 預期 dependency DAG

目前依實際程式結構，預期方向如下：

```text
catalog ─────→ detect ─────→ types
   │                         ↑
   └─────────────────────────┘

request ───────────────────→ types
stream  ───────────────────→ types
runner  ───────────────────→ types
```

外部依賴另計：

- `types.rs` → `data::Tier`（若 Tier 最終仍只在 request 使用，則不必放 types）、`transport::PromptCacheUsage`、`usage_log::*`
- `request.rs` → `data::Tier`、`transport::ChatMessage`
- `runner.rs` → `proxy`、`inflight`、`usage_log`、tokio process/io/time
- `catalog.rs` → serde_json、regex、tokio blocking task

### 已知可能需要的 sibling visibility

若維持此草案，至少要複核：

- `detect::find_binary`：目前 `pub(crate)`，catalog 也需要；是否仍維持 crate visibility或可收窄成 `pub(super)`，取決於外部 caller inventory。
- `detect::hidden_output`：catalog 需要，原本 private，預期可能是 `pub(super)` 候選。
- `request::grok_common_args`：只應留在 request 內，不應為拆檔放寬。
- `stream::usage_event`、`stream::token_count`：只應留在 stream 內。
- `runner::api_error_kind`：production 只在 runner 內；tests 搬到 owner 後不應因此放寬。
- `runner::ChildPidGuard`：只應留在 runner 內。

**正式 visibility 白名單要在施工前 DAG 完成後定案；沒有列入 ledger 的項目不得臨時放寬。**

## 6. 測試搬法

原 `mod tests` 不再整包塞進 `mod.rs`。測試依 production owner 搬到各 implementation 檔的 `#[cfg(test)] mod tests`；只有跨 owner 重複使用且值得共用的 fixture 才新增 `test_support.rs`。

預期 owner：

- binary／version／catalog 類 → `detect.rs` / `catalog.rs`
- flatten／tier／args／session args → `request.rs`
- stream event／usage JSON 樣本 → `stream.rs`
- `api_error_kind`／runner 邊界 → `runner.rs`

硬約束：

1. 原 `#[test] fn` body 逐項保持一致；
2. 只允許拆檔後必要的 import／路徑調整；
3. 不趁搬測試重寫 fixture、合併案例或「整理」斷言；
4. test leaf 數量施工前後必須一致。

## 7. 本案允許與禁止

### 允許

1. `src-tauri/src/cli.rs` → `src-tauri/src/cli/` 的純搬家；
2. 新增 `mod.rs` 與 implementation 檔；
3. 必要 `use`、相對 module path、事前 ledger 核准的最小 visibility plumbing；
4. 測試依 owner 搬家；
5. facade 維持所有有實際 caller 的原 `cli::...` 路徑。

### 禁止

- 改 Claude／Codex／Agy／Grok CLI 旗標；
- 改模型預設、tier mapping、Grok sampling overlay 或 disallowed tools；
- 改 binary 探測位置、probe timeout、catalog timeout；
- 改 streaming JSON 解析規則、fallback 文案或 usage token 算法；
- 改 `run_cli` 的 stdin 60 秒、stall 120 秒、exit 後 800ms 收網、stderr fatal 判定；
- 改 proxy／ANTHROPIC_* 清理、Windows `CREATE_NO_WINDOW`、`kill_on_drop`；
- 改 inflight PID register/unregister 行為；
- 改 prompt-cache usage 落檔格式或 lane／shape 語意；
- 抽象成 provider trait、統一四家 parser、消除重複碼；
- 順手修 unrelated bug／warning。

若施工中發現既有 bug，另立案，不夾帶在本拆分 commit。

## 8. 驗收門檻

每個自然工作段只搬相鄰責任，完成後驗收，不一次吞完整檔案。

最低驗收：

1. `cargo test`
2. `RUSTFLAGS="-Dprivate_interfaces" cargo check --all-targets`
3. `npm run build`
4. production top-level item set 前後一致
5. 除事前 visibility ledger 外，production item 原文逐 item byte-identical
6. test leaf 數量一致、test function body 逐項一致
7. `mod.rs` production 只有 module 宣告／必要 re-export，無 implementation
8. facade re-export = 有 caller 的既有 API，不能漏、不能多掛零 caller
9. `cli.rs` 外 caller 原則上 **0 修改**；若 compiler 證明某條相對路徑無法維持，必須先停下來更新計畫與理由

### 實機／冒煙驗收

因本檔直接控制四家 CLI 子程序，結案前至少做不改資料的 smoke：

- CLI 偵測能列出已安裝 provider；
- 設定頁模型 catalog 能讀；
- 至少一條已登入 CLI 做純文字單發，串流能正常收尾；
- 若本機有可用 Claude/Grok session 路徑，再驗一條 open → resume；
- usage log 能留下該次 CLI 呼叫且 token 欄位可解析。

不要求為驗收額外登入所有 provider，也不為拆檔購買／消耗不必要額度。

## 9. 建議工作段

依約以自然工作段施工：

- **A：依賴複核＋baseline** — 45 item 正式 manifest、caller inventory、test manifest、DAG、visibility ledger，更新本計畫為定案版；不改 `.rs`。
- **B：types + detect + catalog** — 先搬 leaf／低層依賴，跑完整 Rust 驗收。
- **C：request + stream** — 搬 provider args 與 parser，保持原 API path。
- **D：runner + tests + facade 切換** — 搬 `run_cli`、測試 owner 化、刪原 `cli.rs`。
- **E：integrity＋完整 CI／smoke** — byte integrity、caller/facade、build/test、實機 CLI 冒煙。

如果某段實際工作量明顯不足一個自然工作段，可以把相鄰且依賴方向一致的下一小段併入；不得因此跨到拆檔之外的重構。

## 10. 分支生命週期

本案所有後續施工只進 `cli-split`。驗收完成後再以單一 squash commit 收回 `main`，之後刪工作分支。

目前狀態：**已立案，尚未施工。下一步是工作段 A 的依賴複核與 baseline。**
