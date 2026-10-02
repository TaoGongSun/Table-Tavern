# menu-keyboard-webkit — ⋯ 選單在 macOS WebKit 鍵盤操作看不到焦點

## 現象
2026-10-02 使用者實機（世界設定頁世界書旁的 ⋯，四項）：滑鼠點開後按 ↑↓ 沒反應；按 ←→ 會選中第一項；之後 ↑↓ 仍沒反應，但先按 ↑ 再按 → 會選到下一個。

## 狀態
- 已進 main、等實機。根因、修法、驗證與最小重現見 [plans/menu-keyboard-webkit.md](../plans/menu-keyboard-webkit.md)。
- 自動驗證：`npm run verify` 全綠；`npm run test:webkit` 9/9 綠（修前 7 敗）；系統 WKWebView 小程式修後每步焦點可見。Sol 審查與驗收共識。
- 設定視窗分頁列同類問題另立 [settings-tabs-focus-visible](../tasks/settings-tabs-focus-visible.md)。

## 下一步
- 使用者實機驗收：實測佇列梯 1 第 2（實機外觀＋VoiceOver 混用），通過即結案。
