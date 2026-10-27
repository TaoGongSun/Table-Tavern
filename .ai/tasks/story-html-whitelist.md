# story-html-whitelist — 訊息顯示照酒館渲染白名單 HTML

## Summary
`src/shared/ui/story-markdown.ts` 的 marked `html` renderer 把所有原始 HTML 一律跳脫成可見文字（`<b>`、`<details>`、卡片自訂標籤如 `<核心记忆>`）。ST 是 showdown 放行原始 HTML、DOMPurify 白名單消毒：認得的標籤渲染、不認得的剝標籤留內容。card-format-html-comment-shown 查證時拆出（該案只處理註解）。

## Next action
未排程；開工先比對 ST DOMPurify 設定與我們 CSP／安全策略，定白名單與自訂標籤處理。

## 驗收方向
常見白名單標籤照 ST 渲染、自訂標籤不露字也不丟內容；不放寬腳本與事件屬性等安全面。
