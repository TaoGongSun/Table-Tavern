# settings-tabs-focus-visible — 設定視窗分頁列方向鍵移焦在 macOS WebKit 看不到

## 現象（2026-10-02 menu-keyboard-webkit 順手重現）
Playwright WebKit 26.0（與系統 WKWebView 同行為）：滑鼠點一個分頁後按 ←→，焦點有照順序移動（外觀→AI 連線→額度），但每一步都不符合 `:focus-visible`、沒有外框。成因同 [menu-keyboard-webkit](../plans/menu-keyboard-webkit.md)：`SettingsWindow.onTabKeyDown` 攔下方向鍵後用一般 `focus()`，外框只寫在 `:focus-visible`。

## 下一步
照 MoreMenu 的作法改：方向鍵移焦帶 `focusVisible: true`＋`data-focus-ring` 兜底（`base.css` 已有規則），補一支 `*.webkit.test.tsx`。
