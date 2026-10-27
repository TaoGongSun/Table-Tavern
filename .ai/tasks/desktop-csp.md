# desktop-csp — 桌面版補 CSP

## Summary
`src-tauri/tauri.conf.json` 的 CSP 是 null，訊息渲染只靠 DOMPurify 一層；DOMPurify 一旦被繞過，腳本可直接呼叫所有 Tauri 指令。網頁版已有 CSP。story-html-whitelist 查證時發現。

## 已知線索
- 補 CSP 會碰到 React 行內 style（`style-src` 要不要 `'unsafe-inline'`）、卡片介面 iframe、外部圖片、devtools 等，需逐項盤點。

## Next action
未排程；開工先盤點 app 實際需要的來源。

## 驗收方向
桌面版有 CSP 且 app 全功能正常（含卡片介面、生圖顯示）。
