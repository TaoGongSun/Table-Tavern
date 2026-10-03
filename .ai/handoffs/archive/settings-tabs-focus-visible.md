# settings-tabs-focus-visible — 設定視窗分頁列方向鍵移焦在 macOS WebKit 看不到

## 現象（2026-10-02 menu-keyboard-webkit 順手重現）
Playwright WebKit 26.0（與系統 WKWebView 同行為）：滑鼠點一個分頁後按 ←→，焦點有照順序移動，但每一步都不符合 `:focus-visible`、沒有外框。成因同 [menu-keyboard-webkit](../../plans/menu-keyboard-webkit.md)。

## 狀態
- 已結案進 main（2026-10-03）：做法、驗證與結論見 [plans/settings-tabs-focus-visible.md](../../plans/settings-tabs-focus-visible.md)。
- 實機項目在[實測佇列](../../reference/verification-queue.md)梯 1 第 3。
- Tab 順序另立 [settings-tabs-tab-order](../../plans/settings-tabs-tab-order.md)〔作者裁決 2026-10-03〕。
