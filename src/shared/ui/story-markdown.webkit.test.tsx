// 真實 WebKit（Tauri macOS 的引擎）驗 DOMPurify 這道出口本身：npm run test:webkit。
// happy-dom 跑 DOMPurify 會拆掉第一個元素、放過後面的事件屬性（3.4.12–3.4.16 皆然），
// story-markdown.test.ts 只靠 renderer 先轉義原始 HTML 才過，驗不到這一層。
import DOMPurify from "dompurify";
import { expect, it } from "vitest";
import { STORY_MARKDOWN_ALLOWED_ATTR, STORY_MARKDOWN_ALLOWED_TAGS } from "./story-markdown";

it("sanitizer config strips handlers, non-allowlisted tags and script URIs on its own", () => {
  const html = DOMPurify.sanitize(
    '<p onclick="x" style="color:red" class="c">字</p><img src="javascript:alert(1)" onerror="x" alt="a">' +
      '<a href="https://e.com">連結</a><svg><script>alert(1)</script></svg><iframe src="https://e.com"></iframe>',
    { ALLOWED_TAGS: STORY_MARKDOWN_ALLOWED_TAGS, ALLOWED_ATTR: STORY_MARKDOWN_ALLOWED_ATTR },
  );

  expect(html).toBe('<p>字</p><img alt="a">連結');
});
