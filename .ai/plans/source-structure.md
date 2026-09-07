# source-structure — 前端目錄歸位與長期結構規範

## 目的

把 `src/` 根層平鋪的功能模組歸位到功能資料夾，並留下一道機器擋得住的規則，讓往後新增檔案不會再退回「往根目錄丟」的狀態。搬乾淨是副產品，**防復發才是交付物**〔作者裁決 2026-09-07〕。

本案 structure-only：允許改 path、import、module 宣告，不改功能、不改資料格式、不改 UI 行為、不順手重構 production logic。

## 範圍〔作者裁決 2026-09-07〕

**本案做**：`src/` 根層 17 個模組＋11 個測試＋`views/world-editor/` 7 檔＝35 檔歸位；`scripts/check-structure.mjs` 接上 `npm run verify`；短版 `docs/STRUCTURE.md`；`docs/ARCHITECTURE.md` 的 tree 校正；`.ai/` 現行文件的路徑機械更新。

**切出去另立三案**（見 `.ai/tasks/`）：`rust-module-homing`、`ai-workspace-tidy`、`view-layer-homing`。

**不等既有工作線**：`interface-card-panel` 與 `interface-takeover-spike` 都只剩實測、沒有進行中的編輯，因此 `interface-card.ts`、`refactor-shell.ts`、`RefactorResultDialog.tsx` 照搬，搬完更新那兩條 handoff 的路徑讓實測能接手〔作者裁決 2026-09-07〕。

## 目標結構

搬完後 `src/` 根層只剩 `App.tsx`、`App.css`、`main.tsx`、`vite-env.d.ts`。

```text
src/
├── App.tsx / App.css / main.tsx / vite-env.d.ts
├── features/   # 功能群：view + hook + logic + test 同住
├── shared/     # 至少兩個獨立 feature 真的在用的東西
├── controllers/  # 只留跨 feature 的 orchestration
├── views/        # 只留 App 級 layout / dialog / workspace 殼
├── styles/ i18n/ assets/
```

### 逐檔歸位〔模型判斷·未裁決〕

落點依實測 import 關係判定，非依檔名。

| 目的地 | 檔案 |
|---|---|
| `features/refactor/` | `refactor-mode` `refactor-review` `refactor-run` `refactor-shell`（各含 `.test.ts`）、`useRefactorWorkflow.ts`、`RefactorResultDialog.tsx`、`RefactorRunDialogs.tsx` |
| `features/worldbook/` | `worldbook-model.ts`、`useWorldbookEditor.ts`、`WorldbookSection.tsx`、`WorldbookEntryForm.tsx` |
| `features/ai-connection/` | `api-key-check`（含測試）、`cli.ts`、`model-catalog`（含測試）、`model-catalog-store.ts` |
| `features/characters/` | `card-model.ts`、`character-visibility`（含測試） |
| `features/card-interface/` | `interface-card`（含測試） |
| `features/import/` | `import-routing`（含測試） |
| `features/settings/` | `appearance.ts` |
| `shared/contracts/` | `backend-contracts.ts`（25 個跨域 consumer） |
| `shared/ui/` | `drag-reorder.ts`、`story-markdown`（含測試）、`ai-error`（含測試） |

判定備註：

- `ai-error` 的 consumer 是 `CardEditor.tsx` 與 `views/atoms.tsx`，都不屬連線功能，故不進 `ai-connection`。
- `story-markdown` 的唯一 consumer 是 `views/atoms.tsx`（共用元件），故進 `shared/ui/` 而非任何單一 feature。
- `shared/` 只分 `contracts/` 與 `ui/` 兩區；不為單支檔案預建第三個分區。
- `features/settings/`、`features/import/`、`features/card-interface/` 開案時檔案少，但各有明確的下一批住戶（見 `view-layer-homing`），不算空殼。

## 長期規則（正式版寫進 `docs/STRUCTURE.md`）

1. **根層只放入口**：`App.tsx`、`App.css`、`main.tsx`、`vite-env.d.ts`。新增第五個原始碼檔＝架構變更，必須同時改 checker。
2. **升格 feature 要一次搬齊**：某功能的 view、controller、logic、test 只要 owner 單一，就必須同批進 `features/<name>/`。不接受「邏輯搬了、view 留在 views/」的半套升格。
3. **`controllers/` 與 `views/` 只留跨 feature 的東西**。單一 owner 的檔案屬已知待收（`view-layer-homing`），不是規則例外。
4. **`shared/` 不是預設落點**：至少兩個獨立 feature 真的在用，才准搬進去。「以後可能共用」不算。
5. **禁止 `utils/` `helpers/` `misc/` `common/`**，任意深度都禁。不知道放哪＝責任沒想清楚。
6. **測試跟著 production 檔走**，不建鏡像測試樹。
7. **修改既有功能時**發現同伴檔散在錯誤層級，範圍允許就順手歸位；不允許就記進 BACKLOG，不能假裝沒看到。

## check-structure.mjs

掛進 `npm run verify`（`verify.yml` 已經跑 `npm run verify`，CI 不必改）。只擋機器判得準的：

- `src/` 根層出現四個允許檔案以外的原始碼檔。
- 任意深度出現 `utils/` `helpers/` `misc/` `common/`。
- 測試檔位於 `src/` 根層，或不在任何 `features/<x>/`、`shared/<x>/` 內。
- 明顯命名違規（component 非 PascalCase、hook 非 `useXxx`、`foo2.ts` / `temp.ts` 這類過渡名）。

**不提供 allowlist／legacy baseline**〔模型判斷·未裁決〕。理由：本案已把根層清空，不存在需要豁免的舊檔；一旦開放例外清單，CI 紅燈的預設解法會變成「把新檔加進清單」，checker 就退化成橡皮圖章。要改頂層架構就得同時改 checker，讓架構變更在 review 裡藏不住。

測試位置採「須在某個 feature 或 shared 目錄內」的寬判準，不要求與 production 檔同 stem——否則未來的整合測試會被誤殺。

owner 判斷（某支檔案該屬哪個 feature）機器做不到，仍靠規範與 review。不讓 checker 假裝判得出來。

## 施工分段

1. **`shared/` 先立**：`backend-contracts` 25 處引用，先搬它讓後續每一段的 import 改動都在同一套路徑基準上。
2. **`features/refactor/`**：11 檔，本案最大一群，含 `views/world-editor/` 的三支 refactor UI。
3. **`features/worldbook/`**：4 檔，`views/world-editor/` 清空，`views/WorldEditor.tsx` 留作組合兩個 feature 的殼。
4. **其餘 features**：ai-connection、characters、card-interface、import、settings。
5. **checker ＋ 文件**：`check-structure.mjs`、`docs/STRUCTURE.md`、`ARCHITECTURE.md` tree 校正、`CLAUDE.md` 加一行指向 STRUCTURE.md。
6. **`.ai/` 路徑更新**：17 條現行 handoff ＋ `reference/verification-queue.md`。`handoffs/archive/`、`history/`、`DONE.md` 不動。

每段獨立可驗、可 review。

## 驗證

每段跑 `npm run verify`（tsc ＋ vitest ＋ check:i18n ＋ 本案新增的 check:structure）。全案結束另做四項機械檢查：

- 無殘留舊 import 路徑（grep 舊檔名的相對路徑）。
- 測試檔數仍為 11，vitest 收集到的測試數不變。
- `.ai/` 現行文件無指向已搬走路徑的連結。
- `docs/ARCHITECTURE.md` 的 tree 與實際一致。

## 硬限制

- 不改產品行為、不改資料格式、不重寫函式 body。
- 不趁搬家改命名、格式、文案、演算法。
- 一段一驗，不一次搬完整個 `src/`。
- 盤點結果與本表不符時以實際 import 關係為準，並在施工紀錄寫明調整原因。
