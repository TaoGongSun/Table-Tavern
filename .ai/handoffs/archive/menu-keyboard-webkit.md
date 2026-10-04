# menu-keyboard-webkit — ⋯ 選單在 macOS WebKit 鍵盤操作看不到焦點

## 現象
2026-10-02 使用者實機（世界設定頁世界書旁的 ⋯，四項）：滑鼠點開後按 ↑↓ 沒反應；按 ←→ 會選中第一項；之後 ↑↓ 仍沒反應，但先按 ↑ 再按 → 會選到下一個。

## 狀態
- 已進 main。根因、修法、驗證與最小重現見 [plans/menu-keyboard-webkit.md](../../plans/menu-keyboard-webkit.md)。
- 自動驗證：`npm run verify` 全綠；`npm run test:webkit` 9/9 綠（修前 7 敗）；系統 WKWebView 小程式修後每步焦點可見。Sol 審查與驗收共識。
- 設定視窗分頁列同類問題另立 [settings-tabs-focus-visible](settings-tabs-focus-visible.md)。
- 2026-10-04 測試通道代測（獨立 root、零 AI 派送、正式目錄 hash 不變）：世界設定頁點文字框→開 ⋯ 第一項有底色、↑↓ 循環含頭尾繞回每步一項有外框＋底色、滑鼠移入換亮項後方向鍵從該項接續、Esc 回 ⋯ 鈕有外框，截圖皆過。
- 通道限制：點擊與按鍵是合成事件，WebKit 不把它當真滑鼠，觸發條件重現不了（對照：同狀態下不帶選項的 `focus()` 照樣命中 `:focus-visible`），所以修前修後在通道裡外觀相同。另證：`:focus-visible` 不成立時加上 `data-focus-ring` 照樣畫出外框，而每次鍵盤移焦都有掛這個屬性。另外 app 視窗不在前景時 `:focus` 不成立，任何外框底色都不顯示，代測要先把視窗叫到前景。
- VoiceOver 混用略過〔作者裁決 2026-10-03：發佈前只保證滑鼠鍵盤並用的一般使用者，純鍵盤／螢幕閱讀器專用的實機驗收不做〕。

## 結案
2026-10-04 接受通道代測＋兜底外框另證結案，不另做真滑鼠實機〔作者裁決 2026-10-04〕。
