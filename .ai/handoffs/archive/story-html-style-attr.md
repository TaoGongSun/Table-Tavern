# story-html-style-attr — 訊息 HTML 放行 style 屬性

Status: 已結案（2026-10-29，進 main；Sol 方案審 1 輪、驗收 1 輪 PASS）。案別：小案，核心估 ≤80 行（不含測試、i18n）；碰信任邊界輸入，故方案送審。

## Summary
story-html-whitelist 只放純排版標籤，不放 style 屬性；卡片用行內 style 設的字色、底色、字重在 app 裡不生效。ST 放行 style。

## 範圍
最小版：style 屬性只放行六項屬性白名單，值只收顏色、關鍵字與受限字級，帶其他函式或跳脫整條丟；網頁版宿主 CSP 加 `style-src-attr 'unsafe-inline'`。規格見 [plans/story-html-style-attr.md](../../plans/story-html-style-attr.md)。

## 已知限制
見方案檔「已知限制」。

## 驗收方向
卡片行內字色／底色照 ST 顯示；不能蓋住 app 介面、不能外連。
