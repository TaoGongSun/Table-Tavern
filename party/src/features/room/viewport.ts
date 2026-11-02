// iOS Safari 鍵盤彈出時不縮版面（100dvh 不變），Safari 改捲整頁去露出游標，標題列與回合列被推出畫面。
// 版面高度跟著可見區域走（App.css 的 --app-height），並把整頁捲動拉回頂端，行動格與送出鈕留在鍵盤上方。
export function followVisualViewport(win: Window) {
  const viewport = win.visualViewport;
  if (!viewport) return;
  const fit = () => {
    win.document.documentElement.style.setProperty("--app-height", `${viewport.height}px`);
    if (win.scrollY !== 0) win.scrollTo(0, 0);
  };
  viewport.addEventListener("resize", fit);
  viewport.addEventListener("scroll", fit);
  fit();
}
