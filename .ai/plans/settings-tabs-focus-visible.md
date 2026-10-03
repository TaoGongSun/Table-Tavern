# settings-tabs-focus-visible — 做法與驗收

交接：[handoffs/archive/settings-tabs-focus-visible.md](../handoffs/archive/settings-tabs-focus-visible.md)

## 做法（Sol 審查共識 2026-10-03）

根因同 [menu-keyboard-webkit](menu-keyboard-webkit.md)：`SettingsWindow.onTabKeyDown` 用一般 `focus()`，前次焦點來自滑鼠時 WebKit 不判 `:focus-visible`。

1. **共用移焦工具**：MoreMenu 的 `focusFrom`、`FocusSource`、`clearFocusRing` 原樣搬到 `src/shared/ui/focus-ring.ts`，MoreMenu 改 import、行為不變；`sourceOfClick` 留在 MoreMenu。第二個真實使用者是 settings feature，符合 `shared/` 成立條件。
2. **分頁列方向鍵**：←→／Home／End 一律 `focusFrom(button, "keyboard")`（焦點沒實際移動的 Home／End 也亮框）；`scrollIntoView` 照舊。
3. **分頁 click 看輸入來源**（寫在分頁自己這邊）：`detail > 0` 的滑鼠點擊，若這顆帶 `data-focus-ring` 或符合 `:focus-visible`，先 `blur()` 再 `focusFrom(node, "pointer")`——點已聚焦的同一顆不換焦點，標記不會掉，WebKit 因方向鍵真按鍵記住的 `:focus-visible` 也不會掉（實測只清標記、或不 blur 直接 `focusFrom(node, "pointer")`，外框都仍在）；`detail === 0`（鍵盤 Enter/Space、輔助技術）保留。
4. **外部切頁的焦點救援**：外部請求傳不出輸入來源，救援一律 `focusFrom(..., "keyboard")`；「焦點仍在視窗內就不搶」照舊。
5. **CSS 兜底**：`settings-window.css` 的 `outline-offset: -2px` 同時給 `.settings-tab[data-focus-ring]:focus`，否則舊引擎吃 `base.css` 的向外外框，會被分頁列 `overflow-x: auto` 裁掉。
6. 不動：滑鼠點分頁的原生焦點、`base.css`。

## 測試

- `src/features/settings/SettingsWindow.webkit.test.tsx`（`npm run test:webkit`，真 WebKit 26.0、真輸入、載入產品 CSS）：先滑鼠點分頁建立觸發條件，然後
  - → ← End Home 每步焦點落在預期分頁且外框可見；Home 已在第一項、End 已在最後一項也亮框；
  - 方向鍵移焦後滑鼠點同一項／另一項：標記與外框都清掉；
  - 只有原生 `:focus-visible`（字母鍵觸發、無標記）時滑鼠點同一顆清框；舊引擎模式點同一顆清框；
  - Enter／Space 啟用分頁後仍有外框；捨棄確認按取消（斷言 confirm 被呼叫、取消後再按會重問）、busy 阻擋時選中分頁與焦點各自正確；
  - 外部切頁卸載聚焦欄位：救援帶框；焦點仍在視窗內不搶；
  - 舊引擎（剝掉 `focusVisible` 的 `focus`）＋窄視窗：第一項、最後一項捲進可視範圍，外框畫在分頁內（outline-offset＋寬度 ≤ 0、分頁在分頁列可視框內），另存截圖人眼確認。
- `SettingsWindow.test.tsx`（happy-dom）：方向鍵移焦帶 `focusVisible: true`、設 `data-focus-ring`，失焦清掉；滑鼠點擊清、`detail === 0` 保留。
- MoreMenu 的 happy-dom 與 WebKit 測試全部重跑。
- `npm run verify` 全綠。

## 結論（2026-10-03 結案）

- 照上述做法進 main；Sol 驗收同意（含 blur 後重聚焦）。verify 全綠（vitest 616、cargo 769、harness 28），test:webkit 21/21，設定分頁 WebKit 11 案（初版 9 案修前 4 敗）。
- 不 blur 直接 `focusFrom(node, "pointer")` 實測無效，保留 blur；VoiceOver 是否重念排實機。
- Tab 順序（Shift+Tab 在選中分頁多停、Option+Shift+Tab 卡在原處、關閉鈕位置）不併本案，另立 [settings-tabs-tab-order](settings-tabs-tab-order.md)〔作者裁決 2026-10-03〕。
- 實機（外觀＋VoiceOver）排[實測佇列](../reference/verification-queue.md)梯 1 第 3。
