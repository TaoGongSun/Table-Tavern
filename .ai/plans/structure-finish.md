# structure-finish — 落點定案與分包

判準：下次找這段程式碼第一個會去翻哪裡〔作者裁決 2026-10-02：落點由模型決定〕。`views/` 與 `controllers/` 只留把多個 feature 組起來的外殼。

## 落點表（2026-10-02 掃描）

| 檔 | → | 依據 |
|---|---|---|
| `views/CardEditor.tsx`、`views/CardImageDialogs.tsx` | `features/characters/` | 角色卡編輯；測試已在該處 |
| `views/CastRail.tsx`、`CastCards.tsx`、`CastArchive.tsx` | `features/characters/` | 陣容欄＝角色名單呈現 |
| `controllers/useCharacterController.ts` | `features/characters/` | 角色名單、玩家卡、圖快取 |
| `views/WorldEditor.tsx` | `features/worldbook/` | 測試已在該處 |
| `views/EditPage.tsx`＋`CardEditor.test.tsx` 裡的 `describe("EditPage")` | `shared/ui/EditPage.tsx`、`shared/ui/EditPage.test.tsx` | characters 與 worldbook 兩個 feature 共用的頁框 |
| `views/atoms.tsx` 的 `ErrorNote`、`StoryText` | `shared/ui/atoms.tsx` | 六處跨 feature 使用 |
| `views/atoms.tsx` 的 `ActReader` | `features/play/ActReader.tsx` | 單幕閱讀，屬遊玩 |
| `views/PlayView.tsx`、`controllers/useChatController.ts` | `features/play/`（新） | 遊玩畫面與對話流程 |
| `controllers/useSceneActions.ts`＋`features/lobby/scene-actions-lock.test.tsx` | `features/play/` | 換幕／分岔，測試跟著搬 |
| `views/StateBar.tsx`、`controllers/useTableStateController.ts` | `features/table-state/`（新） | 狀態列與其 controller |
| `views/SettingsWindow.tsx`、`SettingsForm.tsx`、`UsageTab.tsx`、`TransportChoice.tsx` | `features/settings/` | 設定頁；測試已在該處 |
| `controllers/useAppPreferencesController.ts` | `features/settings/` | 偏好設定 |
| `views/Onboarding.tsx`、`views/SmartFreeNewModelBanner.tsx` | `features/ai-connection/` | OpenRouter 首設與免費模型提示，邏輯已在該處 |
| `views/ImportDialogs.tsx`、`controllers/useImportController.ts` | `features/import/` | 匯入 |
| `views/CardInterfaceOverlay.tsx`、`controllers/useCardInterfaceController.ts` | `features/card-interface/` | 卡片介面 |
| `views/GenerateTableDialog.tsx` | `features/lobby/` | 一句話開桌＝開新桌 |

留原地（組合外殼）：`views/AppWorkspace.tsx`、`AppDialogs.tsx`、`MainView.tsx`、`TableToolbar.tsx`、`controllers/useWorkspaceNavigationController.ts`。

Rust：`data/worldbook.rs`、`transport/state_view.rs` 內嵌測試拆成 `data/worldbook/tests.rs`、`transport/state_view/tests.rs`；父模組留 `#[cfg(test)] mod tests;`，不動可見度。

## 關卡

`check-structure.mjs` 加 `views/` 與 `controllers/` 的合法外殼清單（同根層 `ROOT_ALLOWED` 性質：列出架構上的固定位置，不是逐檔豁免違規的 baseline）。以完整相對路徑比對、涵蓋所有後代，子資料夾不能繞過；驗收含一個反例（暫放 `views/x/Foo.tsx` 要紅）。checker 開頭與 STRUCTURE.md「沒有 allowlist」的措辭改成「只列架構固定位置，禁止逐檔豁免」。同步更新 `docs/STRUCTURE.md`、`docs/ARCHITECTURE.md`。

## 分包

- 包 1：characters、worldbook、`shared/ui/EditPage`（含測試）、atoms 拆分；`features/play/ActReader.tsx` 在此先建，是包 3 的前置。
- 包 2：settings、ai-connection、import、card-interface。
- 包 3：play、table-state、lobby、checker 白名單與文件。
- 包 4：Rust 兩支拆測試（主線直接做）。

## 做法

- structure-only：不改邏輯、命名、文案；只改 import 路徑與搬移。atoms 拆檔只搬函式，內容不動。
- `git mv` 保留歷史；CSS import 路徑一併改。
- 每包 `npm run verify` 全綠，vitest 測試案例總數與 cargo test 數不變。
- 現行文件（`docs/`、`.ai/handoffs/`、`.ai/reference/`、`.ai/plans/` 中未結案者）的舊路徑機械更新；`archive/`、`history/`、`DONE.md` 不動。
