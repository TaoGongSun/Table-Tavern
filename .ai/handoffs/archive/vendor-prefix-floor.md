# vendor-prefix-floor — 只中到 CLI／供應商自帶底線的輪次，不該報成已省

## 現況

已結案合併進 main。規格與定案見 [plans/vendor-prefix-floor.md](../../plans/vendor-prefix-floor.md)。

- 讀帳時用 `usage/floor.rs` 從全檔分組認底線，`usage/report.rs` 把只中到底線的輪次改判 `vendor-only`（API 為 `vendor-only-api`），省額每輪先扣掉自帶段（B2，見 vendor-floor-b2）、輸入量留在頭條分母；claude 通道不認底線。
- 帳本加兩個可選欄：CLI 呼叫的 `cli_version`、API 呼叫的 `endpoint`（`主機:連接埠#路徑雜湊`）；作者同意加〔作者裁決 2026-10-28〕。
- 額度分頁：範圍內全是只中自帶時，頭條改標「只中到 CLI 自帶部分」／「只中到供應商固定開頭」；認得出底線時命中條分三段（我方命中〔估〕／自帶／全額）；格子黃燈。
- Sol、Grok 驗收 DONE。

## Constraints

- 只動 `usage_report` 的判定與前端顯示，不動組裝與傳輸；帳本只加可選欄〔作者裁決 2026-10-28〕。
- 不與 no-cache-model-optout 共用底線計算（該案門檻不同、不讀 JSONL 做決策）〔模型判斷·未裁決〕。
