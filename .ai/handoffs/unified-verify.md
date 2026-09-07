# unified-verify — 統一驗證入口與 CI

分支：`repo-hygiene/verify`（立案基準 main / 016133c）
規格與分段：[.ai/plans/unified-verify.md](../plans/unified-verify.md)

## 現在成立的狀態

**工作段 1（本地入口）完成。** `scripts/verify.mjs` + `npm run verify`，一條命令依序跑
cargo fmt --check → vitest → check:i18n → build → cargo check → cargo test，Rust 三項
cwd=src-tauri。順序定案：fmt 提到最前面（不讀 dist，幾秒攔格式錯誤）；cargo check／test
留在 build 之後（generate_context! 編譯期要讀前端產物）。
macOS 實跑全綠（vitest 157、cargo 536，exit 0）；故意弄壞 main.rs 格式後第 1 步即停、
exit 1、後 5 步未執行。未納入 ignored real-install smoke 與 clippy。

**工作段 2（CI 接軌）完成。** `ci-windows-verify.yml` 前半段改成 `npm ci` + `npm run verify`，
不再自己抄一份 build／cargo test；toolchain 補 `components: rustfmt`。real-install smoke、
smoke logs artifact、`workflow_dispatch` 維持原樣。
Windows 手動觸發第四跑全綠（run 34080940899，13 分鐘：verify 330s、smoke 331s、
冷 Rust cache 另計；下次 cache 命中會更短）。

前三跑各抓出一個只在 Windows 出現的既有問題，都不是 verify 本身造成的，是這些檢查第一次
上 Windows 才浮出來：

1. `spawnSync npm.cmd EINVAL` — Node 18.20 起禁止 spawn 直接跑 .cmd。npm 三步改帶
   `shell: true`（cargo 是 .exe，維持直接 spawn）。
2. `check-i18n.mjs` 的 `URL.pathname` 在 Windows 是 `/D:/…`，join 後變 `D:\D:\…`。
   改 `fileURLToPath`。
3. 同檔 `await import()` 吃 `C:\…` 絕對路徑，ESM loader 不收。改 `pathToFileURL().href`。

**工作段 3（拆兩支＋自動觸發）完成**〔作者裁決 2026-09-07：拆兩支〕。

- `verify.yml`（新）：`pull_request`、`push: main`、手動皆可，只跑 `npm run verify`。
  windows-latest（Tauri 的 Linux 系統依賴不必為此引入；Windows 路徑問題也繼續守得住）。
  分支 push 實測全綠（run 34082056404，整輪 7 分鐘、verify step 321s）。
- `windows-install-smoke.yml`（原 `ci-windows-verify.yml` 改名）：只留 real-install smoke
  與 log artifact，維持 `workflow_dispatch`；前置 `npm ci` + `npm run build`。

`workflow_dispatch` 只認得預設分支上的 workflow，所以這兩支在分支上按不動（HTTP 404）。
verify.yml 是靠暫時把本分支加進 push 清單來實測的，驗完已移除。

## 下一步

進 main 後手動按一次 `Windows real-install smoke` 確認改名與前置調整無誤，本案即可結案。
結案照專案規約整理分支歷史（含把 `tmp:` 那兩筆壓掉）再合併。

## 範圍外

`src/` 目錄美化／搬檔不在本案，另立案處理〔作者裁決 2026-09-07〕。
