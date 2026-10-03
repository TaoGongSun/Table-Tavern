# sanitize-test-jsdom — 消毒測試改 jsdom、vitest 不掃 .claude/

**狀態：已結案（2026-10-03），Sol 審過做法也驗收同意。**

## 定案

1. **jsdom**：devDependency 加 `jsdom@^29.1.1`，lockfile 同步更新，用 `npm ci` 驗過。選 29 的原因：CI 跑 Node 22，29 要求 `^22.13.0`，30 要求 `^22.22.2`，29 比較寬鬆。只有 `story-markdown.test.ts` 用檔頭註解改成 jsdom，全域 environment 不動。
2. **出口函式**：`story-markdown.ts` 把 DOMPurify 呼叫抽成匯出函式 `sanitizeStoryHtml`，`renderStoryMarkdown` 改呼叫它。jsdom 測試涵蓋四項：
   - 重現字串的輸出要與字串完全一致；
   - https 圖片要保留；
   - 混淆過的 javascript URI 要剝掉（大小寫混用、十進位與十六進位字元實體、tab、控制字元、前導空白換行）；
   - sentinel：spy 讓 `DOMPurify.sanitize` 回傳 sentinel，斷言 `renderStoryMarkdown` 回傳的就是它，而且呼叫時帶的是正式設定，防止 renderer 繞過消毒層。
   WebKit 測試改呼叫同一個函式，留作真引擎對照，不進 verify。
3. **斷言方式**：現在的 fixture 都很簡單，用精確字串比對。以後如果加畸形 HTML 或 SVG 的案，改成驗 DOM 結構。
4. **排除 `.claude/`**：`vitest.config.ts` 的 exclude 寫成 `[...configDefaults.exclude, "**/*.webkit.test.tsx", "**/.claude/**"]`。verify 的其他步驟本來就不掃這裡：
   - structure 只走 `src/`；
   - i18n 只讀 `src/i18n`；
   - tsc 的 include 只有 `src`；
   - vite build 從 index.html 出發；
   - cargo 在 `src-tauri/`；
   - webkit config 的 include 只限 `src/**`。

## 實證（2026-10-03）

- 新測試檔換回 happy-dom 跑：3 案失敗，分別是重現字串、https 圖片、混淆 URI，exit 1。
- 改壞設定，都是 exit 1：
  - `ALLOWED_ATTR` 加 `onclick`：重現字串、sentinel 兩案失敗；
  - 拿掉 `ALLOWED_TAGS`：重現字串、sentinel 兩案失敗；
  - 拿掉 `ALLOWED_ATTR`：重現字串、sentinel 兩案失敗；
  - `renderStoryMarkdown` 跳過消毒：只有 sentinel 一案失敗。
- `.claude/worktrees/` 放一個必定失敗的假測試：
  - 拿掉排除時，它被收進來並失敗，結果是 1 failed / 619，exit 1；
  - 加上排除後，它不會被收進來，結果是 618 passed，exit 0；
  - 驗完已刪除假檔。
- Node 22：本機是 Node 26。jsdom 29 要求 Node ≥22.13，`setup-node` 指定 22 會裝最新的 22.x。進 main 的那次 push 會自動跑 verify.yml（Windows，Node 22），結果以它為準。
