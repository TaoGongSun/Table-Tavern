# story-html-style-attr — 訊息 HTML 放行 style 屬性

## Summary
story-html-whitelist 只放純排版標籤，不放 style 屬性；卡片用行內 style 設的字色、底色、大小在桌面版不生效。ST 放行 style。

## 已知線索
- 送審（story-html-whitelist，Sol／Grok）結論：只擋 `url()` 與 `position:fixed` 不夠，還有 `image-set()`、`content:url()`、`absolute`／`sticky` 蓋版；需 CSS 消毒（Marinara 用 postcss）或 `contain: paint` 一類圍堵。
- 網頁版 CSP `style-src 'self'` 會讓行內 style 無效，兩版不一致，要嘛放寬 CSP、要嘛接受差異。
- 桌面版 CSP 是 null（見 desktop-csp），放寬前要先想清楚第二層。

## Next action
2026-10-29 輕防重審：縮小〔模型判斷·未裁決〕。前置 story-html-whitelist 已結案，可以排程。最小版＝style 屬性只放行少數屬性白名單（color、background-color、font-weight、font-style、text-decoration、有上下限的 font-size），值只收顏色字面值與關鍵字，帶任何函式（url()、image-set() 等）整條丟棄；不做通用 CSS 消毒、不放定位類屬性。蓋版與外連的面因此不存在，不必等 desktop-csp。
刪掉的部分成已知限制：排版類行內 style 不生效；網頁版受 CSP `style-src 'self'` 限制行內 style 不生效，兩版有差。

## 驗收方向
卡片行內字色／底色照 ST 顯示；不能蓋住 app 介面、不能外連。
