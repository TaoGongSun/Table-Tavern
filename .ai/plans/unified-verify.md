# 統一 verify / CI：repo hygiene 立案與施工計畫

分支：`repo-hygiene/verify`  
立案基準：`main` / `016133c233768aca5b534fb9f8a46a5e9d61db30`  
立案日期：2026-09-07

## 0. 立案目的

本案處理 repo hygiene 中的「統一驗證入口」：先把目前分散在 npm、i18n、前端 build 與 Rust 的日常驗證收斂成一個本地命令，目標使用方式為：

```bash
npm run verify
```

這一階段的核心不是增加新的品質門檻，而是把**專案現在已經真的需要做的檢查**整理成一個可重複、可失敗即停、跨 macOS / Windows 可用的入口。之後每完成一輪拆分或重構，本地不必再人工記住應該跑哪些命令。

CI 是第二階段。本案不會一開始就把所有檢查綁到每次 push / PR，更不會把既有 Windows 實機安裝 smoke、Tauri unsigned build 與一般日常 verify 混成同一條巨大 workflow。

## 1. 目前 repo 狀況

### 1.1 npm scripts

目前根目錄 `package.json` 已有：

- `npm run build` → `tsc && vite build`
- `npm test` → `vitest run`
- `npm run check:i18n` → `node scripts/check-i18n.mjs`
- `npm run dev`
- `npm run preview`
- `npm run tauri`

但沒有一個統一的 `verify` script。

### 1.2 Rust 工作目錄

Rust crate 位於：

```text
src-tauri/
└── Cargo.toml
```

因此 Rust 驗證命令需要在 `src-tauri/` 下執行，而不是假設 repo root 本身就是 Cargo workspace。

目前本案預計納入的 Rust 基礎檢查為：

```bash
cargo fmt --check
cargo check
cargo test
```

第一階段**不把 `cargo clippy -- -D warnings` 自動升格成必要門檻**。如果日後要把 clippy 納入 CI，先獨立確認目前基線是否乾淨，再另行決定；本案先收斂既有驗證，不順手新增一批可能造成 legacy cleanup 的規則。

### 1.3 現有 GitHub Actions

目前 `.github/workflows/` 已有兩支 workflow：

#### `ci-windows-verify.yml`

目前只有：

```yaml
workflow_dispatch:
```

也就是手動觸發。

它會：

1. Windows runner
2. Node 22 + Rust stable
3. `npm ci`
4. `npm run build`
5. `cargo test`
6. 額外跑 ignored 的四套 CLI real-install smoke
7. 收集 smoke logs artifact

這支 workflow 內已有重要註解：`generate_context!` 編譯期需要前端資產，所以 `cargo test` 前必須先產生 `dist`。這代表本地統一 verify 的命令順序不能把 Rust compile/test 放在前端 build 之前。

#### `test-build.yml`

用途是 Windows unsigned Tauri build：

- `workflow_dispatch`
- `test-v*` tag push
- `tauri-apps/tauri-action@v0`
- 上傳 MSI / NSIS artifact

這不是日常 code verify，本案不把它改造成一般 CI，也不把完整 release/bundle build 塞進 `npm run verify`。

## 2. 第一階段定案：本地統一 `npm run verify`

### 2.1 目標命令

第一階段完成後，repo root 應可直接執行：

```bash
npm run verify
```

它負責依序執行：

1. `npm test`
2. `npm run check:i18n`
3. `npm run build`
4. `cargo fmt --check`（cwd = `src-tauri`）
5. `cargo check`（cwd = `src-tauri`）
6. `cargo test`（cwd = `src-tauri`）

`npm run build` 必須在需要編譯 Tauri context 的 Rust command 前完成，以確保 `dist` 已存在。

實作時若確認 `cargo fmt --check` 完全不受 `dist` 影響，可以把它提早到 build 前，以更快攔住純格式錯誤；但 `cargo check` / `cargo test` 不得移到前端 build 之前。最終順序以實際驗證結果為準，並在施工 commit 裡明確記錄。

### 2.2 建議實作形狀

不建議直接在 `package.json` 塞一條很長的：

```text
npm test && npm run check:i18n && ... && cd src-tauri && cargo ...
```

雖然短期可用，但會把：

- root / `src-tauri` cwd 切換
- Windows `.cmd` 呼叫差異
- step 名稱
- exit code 傳遞
- 後續新增／刪除驗證項目

全部擠在 JSON 字串裡。

本案優先採用：

```text
package.json
└── "verify": "node scripts/verify.mjs"

scripts/
└── verify.mjs
```

`verify.mjs` 只做薄薄的 command orchestrator：

- 不引入 npm dependency
- 逐步顯示目前正在跑哪個檢查
- 每一步沿用 child process 的 stdout / stderr
- 任一步非 0 立即停止並回傳非 0
- Rust commands 明確使用 `cwd: src-tauri`
- 正確處理 Windows / macOS 的 npm command 呼叫差異
- 不把驗證邏輯本身重寫進 Node；仍然呼叫現有 npm / Cargo 命令

這樣 `npm run verify` 才能同時成為「人類本地入口」與「CI 入口」，避免本地與 GitHub Actions 各維護一份命令清單。

### 2.3 第一階段刻意不跑的東西

`npm run verify` 不包含：

- `cargo test -- --ignored ... real_install_`
- 真實 CLI 安裝 smoke
- Tauri installer / bundle build
- `tauri-action`
- GitHub artifact upload
- release signing
- `cargo clippy -- -D warnings`

其中 real-install smoke 具有環境副作用且目前本來就被標成 ignored；它應繼續由 Windows 專用 workflow 明確執行，不應讓開發者每次本地 verify 都安裝四套 CLI。

## 3. 第一階段施工範圍

預計只修改：

```text
package.json
scripts/verify.mjs
```

除非施工時發現 repo 現有工具鏈有必要的小型設定缺口，否則第一階段不碰：

- `.github/workflows/*`
- Rust production code
- React / TypeScript production code
- 測試內容
- `.ai/` 其他管理結構

這一段應是一個獨立 commit，讓「本地 verify 能不能成立」可以單獨驗證與回退。

## 4. 第一階段驗證方式

施工後至少確認：

1. `npm run verify` 可以從 repo root 啟動。
2. npm / Cargo 每一層 command 都在正確 cwd 執行。
3. `npm test` 綠燈。
4. `npm run check:i18n` 綠燈。
5. `npm run build` 綠燈並產生 Rust compile/test 所需前端資產。
6. `cargo fmt --check` 綠燈。
7. `cargo check` 綠燈。
8. `cargo test` 綠燈。
9. 任一步失敗時，`npm run verify` 本身必須以非 0 結束，不可吃掉錯誤繼續假綠。
10. 不執行 ignored real-install smoke。

GitHub connector 本身不能取代完整本地 Node/Rust 執行環境；因此第一階段程式可遠端施工，但完成判定需要至少一次真正執行 `npm run verify`。

## 5. 第二階段：讓 CI 使用同一個入口

第一階段本地 `npm run verify` 穩定後，再處理 GitHub Actions。

### 5.1 優先方向：改造既有 `ci-windows-verify.yml`

目前這支 workflow 已經具備：

- Windows runner
- Node 22
- npm cache
- Rust stable
- Rust cache
- `npm ci`
- Rust / Tauri 可運作環境
- ignored real-install smoke

因此第二階段優先考慮把前半段一般驗證改成：

```yaml
- run: npm ci
- run: npm run verify
```

然後**保留後半段 Windows-only real-install smoke 與 log artifact**。

這比新增一支幾乎重複的 Windows workflow 更能達成本案的真正目標：本地與 CI 都使用同一個 verify 定義。

若實作時發現 `npm run verify` 與 real-install smoke 的前置條件、執行時間或 cache 行為互相衝突，再考慮拆成兩個 job／兩支 workflow；不要在立案階段先假設一定要新增 workflow。

### 5.2 觸發條件分階段開放

第二階段也不要一次全面自動化。

建議順序：

#### Phase A — 保持 `workflow_dispatch`

先只讓 GitHub Actions 實際證明：

- `npm run verify` 在 `windows-latest` 可跑
- path / npm command / Cargo cwd 沒有跨平台問題
- real-install smoke 仍正常

這一步綠燈後再考慮自動觸發。

#### Phase B — 再評估 PR / main push

候選觸發：

```yaml
pull_request:
push:
  branches:
    - main
workflow_dispatch:
```

但是否同時打開 PR 與 `main` push，要看 Phase A 的執行時間與穩定度。

如果完整 Windows verify + real-install smoke 太慢，不應為了形式完整把它綁在每次 push；可以考慮：

- 一般 `npm run verify` 作為 PR / main 的自動 job
- real-install smoke 繼續 manual 或較低頻率

這個拆法要等實際 runtime 與穩定性資料再定，不在第一階段預設答案。

## 6. `test-build.yml` 的定位

`test-build.yml` 是「產出 unsigned Windows installer artifact」，不是日常 correctness verify。

本案原則：

- 不刪除
- 不合併進 `npm run verify`
- 不因為有 unified verify 就取消 `test-v*` tag build
- 是否在 build 前多跑 `npm run verify`，屬於第二階段之後的獨立決策

避免把「測試程式碼正確」與「真的做 installer bundle」綁成每次都必須一起跑的重型流程。

## 7. 工作段安排

本案依約 20 分鐘的自然工作段推進，不一次把本地入口、CI trigger、branch protection 全做完。

### 工作段 1 — 本地 unified verify

只做：

- 新增 `scripts/verify.mjs`
- 新增 `npm run verify`
- GitHub 端檢查 diff
- 可執行環境實跑完整 `npm run verify`
- 必要時只修 command orchestration 問題
- 獨立 commit

**這是下一個正式施工段。**

### 工作段 2 — 既有 Windows workflow 接 unified verify

只有工作段 1 綠燈後才做：

- 讓 `ci-windows-verify.yml` 的一般驗證改呼叫 `npm run verify`
- 保留 ignored real-install smoke
- 保留 smoke logs artifact
- 暫時仍只 `workflow_dispatch`
- 真的觸發 GitHub Actions 驗證
- 獨立 commit

### 工作段 3 — CI 自動觸發評估

根據工作段 2 的實際執行時間與穩定度決定：

- 是否加入 `pull_request`
- 是否加入 `push` to `main`
- 是否把一般 verify 與 real-install smoke 拆 job
- 是否需要 path filter

這一段不是「必須全部開」；若收益不夠或 runtime 太重，可以定案繼續只保留手動 Windows smoke，而另設較輕的自動 verify。

## 8. 驗收標準

### 第一階段完成

以下全部成立才算本地 unified verify 完成：

- repo root 有 `npm run verify`
- 不需人工切到 `src-tauri`
- 前端 tests / i18n / build 與 Rust fmt / check / test 全部由同一入口執行
- fail-fast 且 exit code 正確
- macOS 本地可實跑
- 沒有把 ignored real-install smoke 帶進日常本地驗證

### 第二階段完成

以下成立才算 CI 接軌完成：

- GitHub Actions 也呼叫 `npm run verify`，不另外抄一份一般驗證命令清單
- `ci-windows-verify.yml` 原本的 real-install smoke 與 artifact 行為沒有被吃掉
- 至少一次手動 workflow run 完整綠燈
- 才開始考慮 PR / main 自動 trigger

## 9. 風險與施工注意事項

### 9.1 Windows child process

Node `spawn` / `spawnSync` 直接呼叫 `npm` 在 Windows 與 Unix 的執行檔解析不同。施工時必須用跨平台寫法，不以「macOS 本地能跑」當成完成。

### 9.2 `dist` 前置條件

既有 Windows workflow 已證實 Tauri `generate_context!` 需要前端 build 產物。不得把 Rust compile/test 順序改到 `npm run build` 之前後又誤以為 Cargo 本身壞掉。

### 9.3 不讓 unified verify 變成無限擴張清單

第一版只收斂現在真正需要的 baseline。若之後要加入：

- clippy
- frontend lint
- formatting tools
- coverage
- installer build
- E2E

每項都先確認 repo 已有穩定基線，再獨立納入；不要因為有了一個 `verify` 名字就把所有可能想到的檢查一次塞進去。

### 9.4 CI 重複成本

若 PR、main push、manual Windows smoke 都跑相同重型流程，很容易產生重複 runner 時間。自動 trigger 必須在看過實際 runtime 後再決定，不追求「每個事件都跑」的形式完整。

## 10. 明確不做

本案不處理：

- `src/` 目錄美化／搬檔
- `.ai/` 結構重整
- Rust / React 功能重構
- 測試內容重寫
- 全面新增 lint 規則
- 強制 `cargo clippy -- -D warnings`
- branch protection / required status checks
- release signing
- installer 發布流程
- 把 real-install smoke 改成日常本地命令
- 第一個 commit 就自動開啟所有 PR / push CI

## 11. 預期成果

完成本案後，最重要的成果不是「多了一支 workflow」，而是 repo 有一個單一、穩定的驗證契約：

```bash
npm run verify
```

本地開發、拆分工程與 GitHub Actions 都逐步共用這個入口。之後每次完成一個工作段，只要看到這一條綠燈，就不用重新回想前端、i18n、Cargo 到底漏跑了哪一項；Windows-only 的真實安裝 smoke 與 Tauri build 則繼續維持各自清楚的用途，不讓日常驗證被重型工作拖垮。
