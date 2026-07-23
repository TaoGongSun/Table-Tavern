# Task handoff
Task-ID: ui-settings-panel
Updated: 2026-07-23T23:05:00+08:00
Status: in-progress

## Goal
依 NewPlan §9.4：單一設定鈕開設定視窗，內部分頁——外觀（預設頁：文字大小、語言）／AI 連線（key、傳輸層、檔位模型）。原側欄底部的 AI 設定摺疊區與語言下拉移入。

## Current state
程式碼完成（主線 Fable 5 直做）：側欄底改單一「⚙️ 設定」鈕，開 modal 兩分頁；外觀頁含語言下拉＋文字大小（小／標準／大），改了立即生效並寫 config；AI 連線頁是原 Settings 表單原樣搬入。npm build 綠。剩使用者視覺驗收。

## Completed
- App.tsx：
  - `SettingsWindow` 元件：modal overlay＋兩分頁（外觀預設）、Esc 或點背景關閉；外觀頁語言與文字大小走 `onPreference` 即改即存，AI 頁沿用 Settings 元件含儲存鈕
  - `Settings` 元件拆掉 `<details>` 摺疊外殼，改直接渲染表單（內容零變動）
  - `changeLanguage` 一般化為 `changePreference(key, value)`；新增 `text_size` 偏好（small/medium/large → 14/16/18px），useEffect 套在 `document.documentElement` 根字級，rem 版面整體縮放
  - 側欄底部原語言下拉＋AI 設定摺疊區刪除，換單一設定鈕
- App.css：`.modal-overlay/.modal/.tabs/.tab/.tab-active/.modal-close/.settings-open` 新樣式（含 dark mode modal 底色）；刪 `.language-picker`；`:root` line-height 24px → 1.5（文字縮放行距跟著走）
- i18n：新增 settingsBtn／appearanceTab／aiTab／closeBtn／textSize 四鍵（zh＋en）；刪 settingsSummary；onboardCliHint 改指「設定 → AI 連線」

## Verification
- `npm run build`：rc=0（tsc＋vite 綠）
- 後端零改動（偏好走既有 write_config），不需 cargo test（前次 40 綠）

## Working context
- Repo: /Users/pachelo/GitHub/Table-Tavern
- Branch: main

## Remaining
- 使用者視覺驗收：設定鈕開視窗預設外觀頁 → 換語言即時生效 → 文字大小三檔切換全版面縮放 → AI 連線頁儲存正常 → Esc／背景點擊關閉

## Next action
- 使用者驗收通過即結案；若外觀頁日後加主題（release-4）已有明確的家

## Constraints
- 偏好存 config.preferences（language／text_size）；純視覺狀態（側欄寬、桌列表摺疊）維持 localStorage 不進 config
