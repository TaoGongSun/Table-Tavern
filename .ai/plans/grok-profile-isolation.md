# grok-profile-isolation — grok 通道環境隔離

## 病因

grok CLI 有「Claude Code Compatibility」：自動從 `$HOME/.claude` 載入 hooks、skills、plugins、`CLAUDE.md`、permissions，官方 README 明寫無需設定、也沒有可關的開關（`[features] claude_hooks` 是伺服器端 flag，`CLAUDE_CONFIG_DIR` 無效，兩者都實測過）。

實測 `grok inspect` 在 app 的 cli-workspace 下：Hooks 9（全來自 `~/.claude/settings.json`）、Project Instructions 1（使用者的寫碼規約，1798 tokens）、Skills 37。其中一個 Stop hook 對陌生 payload 回 exit 2，grok 0.2.111 會把 Stop hook 失敗當回饋餵回模型續跑 → 旁白無限重寫，燒玩家額度。

同 app 其他通道都有隔離（claude `--safe-mode`、codex `--ignore-user-config`、agy 無此相容面），只有 grok 裸奔。grok CLI 本身沒有隔離旗標，唯一槓桿是環境變數。

## 拍板結論

**app 自帶一套 grok profile，與使用者的終端機 grok 完全不共用。**（2026-08-21 使用者拍板：共用會讓 CLI 與 app 狀態互相混淆；測試期無其他使用者，重登可接受。）

注入的環境（四處共用同一組）：

| 變數 | 值 | 為什麼 |
|---|---|---|
| `HOME` / `USERPROFILE` | `<config_root>/cli-home` | 讓 grok 找不到 `~/.claude` 與 `~/.claude.json` |
| `GROK_HOME` | `<config_root>/grok-home` | grok 官方變數，覆寫設定目錄；玩家在 app 裡登入的憑證存這裡 |

必須注入的四處（少一處就會出現「UI 顯示已登入、實跑未登入」）：

1. 旁白單發 `run_cli`（lib.rs 的 grok 分支）
2. `grok models`（`cli_model_catalog`，設定頁模型下拉與登入判定）
3. mac 安裝／登入腳本的 `grok login` 與探針
4. Windows `InstallSpec` 的 login 與 probe

**不帶 `--reasoning-effort`**：用 grok 官方預設。app 不替玩家決定燒多少額度；要調由設定頁另開選項（未排程）。

**加 `--max-turns 1`**：旁白是單發，封死任何「模型自己多跑幾輪」的額度出血口。

## 驗收

- `grok inspect` 在注入環境下：Hooks 0、Project Instructions 0、Skills 0、Plugins 0、Permissions 0。
- 設定頁的 grok 連結流程能在 app 自己的 profile 完成登入，登入後模型下拉抓得到清單。
- cargo test／vitest／build／`npm run check:i18n` 四項綠。

## 待辦（公開測試前必做）

`grok-home/sessions/` 會隨每次旁白累積，grok 沒有停用持久化的旗標。要加保留上限，且必須考慮並行呼叫，不能粗暴刪整個 sessions 目錄。
