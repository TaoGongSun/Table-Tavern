# Table Tavern — Current Architecture

> 現況導覽，更新於 2026-09-06。這份文件只回答「現在程式放哪裡、責任怎麼分」；功能規格與個別決策仍以 `.ai/plans/`、進行中的 `.ai/handoffs/` 與實際程式碼為準。

## 1. 技術棧與入口

Table Tavern 是 **Tauri 2 + Rust** 後端、**Vite + React + TypeScript** 前端。

- 前端入口：`src/main.tsx` → `src/App.tsx`
- Tauri 後端入口：`src-tauri/src/main.rs` → `src-tauri/src/lib.rs`
- `src-tauri/src/lib.rs` 負責註冊 Tauri commands；實際功能依 domain 分散在 `commands/` 與其下層模組。

`App.tsx` 是 composition root：保留跨 domain 的桌次切換、匯入復原、重構套用後刷新等協調，不再承擔所有功能實作。

## 2. 前端

```text
src/
├── App.tsx                 # composition root / 跨域協調
├── controllers/            # 狀態與操作流程
├── views/                  # React 畫面元件
│   └── world-editor/       # 世界編輯器／AI 重構子模組
├── styles/                 # 依 UI 區域拆分的全域 CSS
├── i18n/                   # 十語系字典
└── *.ts / *.test.ts        # 共用 model、routing、formatting 與單元測試
```

主要 controller 已按責任拆開，例如：

- `useAppPreferencesController`：設定、外觀與語言偏好
- `useCharacterController`：角色資料與編輯
- `useChatController`：聊天流程
- `useImportController`：卡片／世界書匯入
- `useTableStateController`：狀態樹
- `useWorkspaceNavigationController`：主工作區導覽
- `useSceneActions`：換幕／分岔／退幕等場景操作
- `useCardInterfaceController`：介面卡渲染與相關狀態

`src/App.css` 現在只作為 CSS facade，實際規則在 `src/styles/`。

## 3. Rust 後端

```text
src-tauri/src/
├── commands/               # Tauri command 邊界
├── data/                   # 世界、角色、場景、狀態、路徑與持久化
├── transport/              # prompt/context 組裝、API client、回覆處理
├── cli/                    # CLI 偵測、執行、request/stream
├── lanes.rs                # CLI 共線／session 延續與快取線管理
├── mechanism/              # 機制解析、規則、狀態樹、trigger、ledger
├── import/                 # 角色卡／世界書／介面／機制匯入
├── refactor/               # 重構套用與介面層
├── refactor_ai/            # AI 重構 survey / expand / rewrite / parse
├── refactor_assemble.rs    # AI 重構結果組裝與稽核
├── receipts.rs             # 匯入／重構復原收據
├── usage_log.rs            # 用量事件紀錄
└── usage_report.rs         # 用量彙整與呈現資料
```

幾個曾經很大的模組已拆成 facade + 子模組，包括 `cli/`、`data/scene/`、`mechanism/`、`import/`、`refactor/`、`refactor_ai/`、`transport/`。目前不要再以「檔案大」本身作為拆分理由；依 `CLAUDE.md`，應優先看責任是否真的混在一起。

## 4. 傳輸層現況

目前不是 2026-07 起手文件裡的「所有 CLI 都無狀態單發」架構。

- API 與 CLI 共用上層的 context / prompt 組裝概念。
- API 路徑走 `transport/`。
- CLI 執行細節在 `cli/`。
- Claude／Grok 等需要 session 延續與快取線管理的路徑由 `lanes.rs` 處理；線狀態、正典 transcript 對齊、素材漂移、重開／續聊與失敗降級都在這一層。
- 角色私設與世界書可見性是架構級資料邊界，修改 transport / lane / import 時不能只看 prompt 字串是否能送出。

相關細節常會隨供應商與快取策略演進；開工前應先讀對應 `.ai/handoffs/` 與 `.ai/plans/`，不要從舊產品文件反推現在行為。

## 5. 專案工作文件

`.ai/` 是目前的工程記憶入口：

- `.ai/BACKLOG.md`：未開工
- `.ai/HANDOFF.md`：進行中工作線索引
- `.ai/handoffs/<id>.md`：各工作線目前仍成立的狀態與下一步
- `.ai/plans/<id>.md`：規格、施工計畫、驗收基準
- `.ai/reference/verification-queue.md`：等實機驗收的項目
- `.ai/DONE.md`、`.ai/history/`：舊 CLI 時代封存，只讀

協作與分支規則看根目錄 `CLAUDE.md`。

## 6. 建置與驗證

前端常用：

```bash
npm ci
npm run build
npm test
npm run check:i18n
```

Rust 測試：

```bash
cd src-tauri
cargo test
```

Tauri 的 `generate_context!` 會在編譯期吃前端資產；CI 因此會先 `npm run build` 再跑 `cargo test`。

Windows 另有 `.github/workflows/ci-windows-verify.yml`，包含原生 `cargo test` 與四個 CLI 的 ignored real-install smoke。

## 7. 哪些文件是歷史資料

- `NewPlan.md`：2026-07 起始產品方案，仍可用來理解產品最初方向，但不少實作與供應商策略已被後續 `.ai/plans/` 與程式碼取代；不要把每一節都當成現行技術規格。
- `docs/archive/KICKOFF.md`：2026-07-18 從零重寫時的工程起手快照，已正式封存，不再作為開工依據。

要了解「現在怎麼做」，先從本文件、`.ai/README.md`、`.ai/HANDOFF.md` 與對應 plan/handoff 開始，再回到程式碼確認。
