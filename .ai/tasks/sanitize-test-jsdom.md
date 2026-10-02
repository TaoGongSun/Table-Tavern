# sanitize-test-jsdom — 消毒測試改 jsdom、vitest 不掃 .claude/

## 要做的事
1. HTML 消毒測試改用 jsdom 環境，讓 `npm run verify` 驗得到 DOMPurify 那層。現況：happy-dom 下 DOMPurify 不消毒，`story-markdown.test.ts` 能過全靠 marked renderer 先轉義原始 HTML；DOMPurify 只在 `story-markdown.webkit.test.tsx`（`npm run test:webkit`，不在 verify）有驗。選 jsdom，不把 test:webkit 納入 verify（verify 變慢、要 WebKit 環境）。
2. `vitest.config.ts` exclude 掉 `.claude/**`，並確認 verify 第一步的結構檢查等其他工具也不掃進去。現況：`.claude/worktrees/` 裡的子代理工作樹會被一起跑，vitest 數字從 504 灌成 1514。工作樹本身不要刪（可能有施工中的子代理）。

## 重現案例（dompurify-xss-upgrade 時查到，3.4.12 與 3.4.16 皆然）
happy-dom 下 `DOMPurify.sanitize('<p onclick="x">字</p><img src="javascript:alert(1)" onerror="x" alt="a">', { ALLOWED_TAGS: ["p","img"], ALLOWED_ATTR: ["alt","src"] })` 回 `字<img src="javascript:alert(1)" onerror="x" alt="a">`——拆掉第一個元素、放過事件屬性與 javascript: src。真 WebKit 下正確。
