# Task
Task-ID: view-layer-homing
Title: views/ 與 controllers/ 的單一功能檔案歸位到 features/
Status: backlog
Created: 2026-09-07T00:00:00+08:00
Updated: 2026-09-07T00:00:00+08:00

## Summary

[source-structure](../plans/source-structure.md) 把 `src/` 根層的模組歸位進 `features/`，但 `views/`（16 檔）與 `controllers/`（8 檔）裡還有約 25 支其實只服務單一功能的檔案沒收：`CardEditor.tsx`、`SettingsForm.tsx`／`SettingsWindow.tsx`／`UsageTab.tsx`、`PlayView.tsx`、`ImportDialogs.tsx`、`CardInterfaceOverlay.tsx`，以及 `useCharacterController`、`useImportController`、`useChatController`、`useAppPreferencesController` 等。

真正跨功能、該留在原地的只有 `MainView`、`AppWorkspace`、`AppDialogs`、`WorkspaceHeader`、`TableSidebar`、`Onboarding`、`useWorkspaceNavigationController`、`useTableStateController`。

source-structure 沒有一起做的理由：一次搬 60 檔，review 只看得到 diff --stat 等於沒 review，而且當時會撞上 interface 那批工作線。這批屬**已知待收**，不是規則例外——`check-structure.mjs` 判不出 view 的 owner，所以這一區在收完之前不能宣稱被 CI 防住。

收完之後才可以加上「`views/` 只准放 composition shell」這條硬規則。

## Next action
- 未排程，排在 `interface-card-panel` 與 `interface-takeover-spike` 收工、`views/` 不再被頻繁改動之後。開工首步＝逐檔掃 consumer 判 owner，一個 feature 一段搬。

## Constraints
- 沿用 source-structure 的硬限制：structure-only，不改行為、不趁搬家改命名或文案。
- 升格要一次搬齊：某功能的 view、controller、logic、test 同批進 `features/<name>/`，不接受半套。
- 一段一驗 `npm run verify`，不一次搬完。
