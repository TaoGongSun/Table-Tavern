# Handoff: claude-compat-endpoint

## Current state
實作完成、雙驗證綠，等使用者實測後結案。

## Completed
- `run_cli` 加 `envs: &[(String, String)]` 參數並套進 spawn（src-tauri/src/cli.rs:544、556）；四個 call site 補參數，claude 分支組 env、其餘傳 `&[]`（src-tauri/src/lib.rs:437-483）。
- env 規則：`preferences["claude_base_url"]` trim 後非空 → `ANTHROPIC_BASE_URL`；`api_keys["claude_compat"]` 非空再加 `ANTHROPIC_AUTH_TOKEN`；空＝空 slice 零行為變化。
- 設定頁 `<details>` 進階區（僅 transport==="claude"、預設收合）：base URL＋password 型 API key 兩欄，接 dirty-check 與儲存（src/App.tsx:234-237、360-361、385-390、588-609）。
- i18n zh／en 四鍵：claudeCompatSummary／BaseUrlLabel／KeyLabel／Hint（src/i18n.ts:82-86、279-283）。

## Verification
- `cargo test`：77 passed; 0 failed（主線本機重跑；codex 環境唯一失敗為其 sandbox 禁 TcpListener 的既有 mock 測試，非本次改動）。
- `npm run build`：✓ built in 415ms。
- diff 全文 read-back 對規格逐條核過（185 行，4 檔）。

## Remaining / Next action
- 使用者實測（見 tasks/claude-compat-endpoint.md Next action），通過即結案。
- 不打包：依約 CI 打包等使用者說了才觸發。

## Constraints
同 tasks 檔；另注意 grok／codex／agy 如日後也要相容端點，沿同一 envs 參數擴充即可。
