// 真實 WebKit（Tauri macOS 的引擎）驗 DOMPurify 這道出口本身：npm run test:webkit。
// happy-dom 跑 DOMPurify 會拆掉第一個元素、放過後面的事件屬性（3.4.12–3.4.16 皆然），
// 所以 story-markdown.test.ts 改用 jsdom 驗同一個函式；這支留著對照真引擎。
import { expect, it } from "vitest";
import { sanitizeStoryHtml } from "./story-markdown";

it("sanitizer config strips handlers, non-allowlisted tags and script URIs on its own", () => {
  const html = sanitizeStoryHtml(
    '<p onclick="x" style="color:red" class="c">字</p><img src="javascript:alert(1)" onerror="x" alt="a">' +
      '<a href="https://e.com">連結</a><svg><script>alert(1)</script></svg><iframe src="https://e.com"></iframe>',
  );

  expect(html).toBe('<p>字</p><img alt="a">連結');
});
