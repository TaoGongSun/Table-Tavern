# settings-tabs-tab-order — 設定分頁改普通按鈕列

交接：[handoffs/archive/settings-tabs-tab-order.md](../handoffs/archive/settings-tabs-tab-order.md)

## 方向〔作者裁決 2026-10-03〕

設定分頁列不做鍵盤便利功能，也不刻意擋鍵盤：改成普通按鈕列，鍵盤只靠瀏覽器原生 Tab／Enter／Space，盡量簡單。

## 做法（Sol 審查同意）

- `SettingsWindow.tsx`：拿掉 tablist／tab／tabpanel、aria-selected、aria-controls、roving tabIndex、方向鍵處理與點擊重聚焦；外層 `<nav aria-labelledby="settings-title">`，五顆 `<button type="button">`，選中那顆 `aria-current="true"`；面板改一般 div。settings-body 初始焦點、面板的捲動用 tabIndex 保留。
- 外部切頁的焦點救援保留、改一般 `.focus()`，「焦點仍在視窗內就不搶」照舊，不強制外框。
- CSS：選中樣式改看 `[aria-current="true"]`；`:focus-visible` 的 `outline-offset: -2px` 保留（防 overflow-x 裁外框）。
- `shared/ui/focus-ring.ts` 只剩 MoreMenu 使用，原樣併回 MoreMenu 後刪檔。
- 第 2 項（滑鼠點按鈕文字後 Shift+Tab 原地不動）：不做 workaround、不追查〔作者裁決 2026-10-03〕。
- 第 3 項（關閉鈕在分頁列與內容之間）：順序不改〔作者裁決 2026-10-03〕。
- macOS 預設一般 Tab 不停普通按鈕，要開系統「鍵盤導覽」或用 Option+Tab 才走得到分頁；這是改用普通按鈕後的原生差異，不是回歸。不另加 WKWebView 實測〔模型判斷·未裁決〕：依作者「盡量簡單」的方向，Sol 原建議加。

## 測試

- WebKit（`src/features/settings/SettingsWindow.webkit.test.tsx`，macOS 一般 Tab 不停按鈕，一律用 Option+Tab）：Option+Tab 逐顆走過五顆再到關閉鈕、反向走回；滑鼠點切頁；Enter／Space 經 click 切頁且留外框；Enter／Space 經守門（取消、確認窗叫不起來、同意）；儲存中拒絕切頁；外部切頁救援與不搶焦點；窄視窗 Tab 到頭尾按鈕會捲進可視範圍、外框畫在按鈕內；背景隔離（四種按鍵各走完整圈、背景 `focus()` 失敗、Esc 關窗焦點交還）。每次讀選中頁都斷言恰好一顆 `aria-current`。
- happy-dom（`SettingsWindow.test.tsx`）：普通按鈕列語意（無 role、無 tabindex、恰好一顆 aria-current）、方向鍵不攔。
- MoreMenu happy-dom 與 WebKit 測試照跑。

## 結論（2026-10-03 結案）

照上述做法進 main；Sol 驗收同意。verify 全綠（vitest 614、cargo 769、harness 28），test:webkit 19/19。不排實機項目。
