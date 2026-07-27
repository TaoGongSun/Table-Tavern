# Task handoff
Task-ID: ui-settings-panel
Updated: 2026-07-24T00:40:00+08:00
Status: done

## Goal
依 NewPlan §9.4：單一設定鈕開設定視窗，內部分頁——外觀（預設頁：文字大小、語言）／AI 連線（key、傳輸層、檔位模型）。原側欄底部的 AI 設定摺疊區與語言下拉移入。

## Current state
程式碼完成（主線 Fable 5 直做）：側欄底改單一「⚙️ 設定」鈕，開 modal 兩分頁；外觀頁含語言下拉＋文字大小；AI 連線頁是原 Settings 表單原樣搬入。使用者初驗回饋後文字大小改五檔（更小10／小12／中14／大16／更大18px，預設「大」＝原視覺大小；偏小取向：大螢幕看長文要小字）。npm build 綠。2026-07-24 使用者複驗五檔通過（附截圖：更小檔全版面縮放正常），結案。

## Completed
- App.tsx：
  - `SettingsWindow` 元件：modal overlay＋兩分頁（外觀預設）、Esc 或點背景關閉；外觀頁語言與文字大小走 `onPreference` 即改即存，AI 頁沿用 Settings 元件含儲存鈕
  - `Settings` 元件拆掉 `<details>` 摺疊外殼，改直接渲染表單（內容零變動）
  - `changeLanguage` 一般化為 `changePreference(key, value)`；新增 `text_size` 偏好（xs/s/m/l/xl → 10/12/14/16/18px，預設 l；未知舊值回退預設），useEffect 套在 `document.documentElement` 根字級，rem 版面整體縮放
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
- 無

## Next action
- 無（任務結案）；若外觀頁日後加主題（release-4）已有明確的家

## Constraints
- 偏好存 config.preferences（language／text_size）；純視覺狀態（側欄寬、桌列表摺疊）維持 localStorage 不進 config
