// 照輸入來源明講焦點要不要可見：WebKit 在「上次焦點來自滑鼠」後，程式移焦不算 :focus-visible，
// 鍵盤造成的移焦就看不到（menu-keyboard-webkit）。MoreMenu 與設定視窗分頁列共用。

/** pointer＝滑鼠／觸控點出來的；keyboard＝其餘（鍵盤、輔助技術、程式觸發的 click），一律顯示焦點框 */
export type FocusSource = "keyboard" | "pointer";

// TS 5.8 的 FocusOptions 還沒有 focusVisible（WebKit 自 Safari 18.4 起支援）
type FocusOptionsWithVisible = FocusOptions & { focusVisible?: boolean };

export function clearFocusRing(this: HTMLElement) {
  delete this.dataset.focusRing;
}

// 不認 focusVisible 的舊引擎靠 data-focus-ring 兜底（base.css 與 :focus-visible 同一條外框規則），失焦即清
export function focusFrom(node: HTMLElement | null | undefined, source: FocusSource) {
  if (!node) return;
  const visible = source === "keyboard";
  if (visible) {
    node.dataset.focusRing = "";
    node.addEventListener("blur", clearFocusRing, { once: true });
  } else {
    delete node.dataset.focusRing;
  }
  const options: FocusOptionsWithVisible = { preventScroll: !visible, focusVisible: visible };
  node.focus(options);
}
