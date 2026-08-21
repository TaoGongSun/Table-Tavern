# grok 通道環境隔離：app 自帶 grok profile，不再吃使用者的 ~/.claude 與 ~/.grok

Status: in-progress

## Summary
grok CLI 會自動載入 `$HOME/.claude` 的 hooks／skills／CLAUDE.md（官方無 opt-out），玩家的 coding Stop hook 因此擋停旁白並讓 grok 無限重寫、燒額度。修法是注入 `HOME`／`USERPROFILE`／`GROK_HOME`，讓 grok 只看得到 app 自己的 profile。拍板結論與驗收清單見 [.ai/plans/grok-profile-isolation.md](../plans/grok-profile-isolation.md)。

## Progress
- 四處環境注入完成：旁白 `run_cli`、`grok models`（`cli_model_catalog`）、mac 安裝腳本的 login／探針、Windows `InstallSpec`。
- `grok_args` 只在無工具旁白加 `--max-turns 1`（生圖要兩輪，不能加）；不帶 `--reasoning-effort`。
- profile 目錄在 unix 下收成 `0700`。
- 自驗全綠：cargo 510／vitest 157／build／`check:i18n` 十語系。
- 實機驗證：以 app 真實路徑跑 `grok inspect`，Hooks 9→0、Project Instructions 1→0、Skills 37→0、Plugins／Permissions 0；`grok models` 在 app profile 下回「You are not authenticated.」，登入流程會照預期觸發。
- Sol 兩輪審查通過（2026-08-21）：第二輪抓到 `list_cli_models` 的 fail-open 與 `--max-turns 1` 誤傷生圖，兩點均已修並補測試。

## Next action
剩使用者在設定頁跑一次 grok 登入（app profile 全新，與終端機 `~/.grok` 不共用），登入後模型下拉抓得到清單、旁白能正常發言即結案。

## Constraints
- 安裝那步刻意不套隔離環境：安裝腳本要把 binary 裝進使用者真正的家目錄。
- 四處必須共用同一組環境，少一處就會出現「UI 顯示已登入、實跑未登入」。
- 設定頁不補登入說明文案（2026-08-21 拍板）。
