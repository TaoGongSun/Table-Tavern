# story-html-style-attr — 訊息 HTML 放行 style 屬性

## Summary
story-html-whitelist 只放純排版標籤，不放 style 屬性；卡片用行內 style 設的字色、底色、大小在桌面版不生效。ST 放行 style。

## 已知線索
- 送審（story-html-whitelist，Sol／Grok）結論：只擋 `url()` 與 `position:fixed` 不夠，還有 `image-set()`、`content:url()`、`absolute`／`sticky` 蓋版；需 CSS 消毒（Marinara 用 postcss）或 `contain: paint` 一類圍堵。
- 網頁版 CSP `style-src 'self'` 會讓行內 style 無效，兩版不一致，要嘛放寬 CSP、要嘛接受差異。
- 桌面版 CSP 是 null（見 desktop-csp），放寬前要先想清楚第二層。

## Next action
未排程；等 story-html-whitelist 結案後評估。

## 驗收方向
卡片行內字色／底色照 ST 顯示；不能蓋住 app 介面、不能外連。
