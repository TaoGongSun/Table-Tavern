// @vitest-environment jsdom
// 不能用 happy-dom：DOMPurify 在 happy-dom 下不消毒（拆掉第一個元素、放過事件屬性與 javascript:），
// 測不到 sanitizeStoryHtml 這層。真 WebKit 對照見 story-markdown.webkit.test.tsx。

import DOMPurify from "dompurify";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  renderStoryMarkdown,
  sanitizeStoryHtml,
  STORY_MARKDOWN_ALLOWED_ATTR,
  STORY_MARKDOWN_ALLOWED_TAGS,
} from "./story-markdown";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("renderStoryMarkdown", () => {
  it("renders story markdown within the shared allowlist", () => {
    const html = renderStoryMarkdown("*動作* **強**\n單一換行\n\n> 引用\n\n- 項目\n\n`code`");

    expect(html).toContain("<em>動作</em>");
    expect(html).toContain("<strong>強</strong>");
    expect(html).toContain("<br>");
    expect(html).toContain("<blockquote>");
    expect(html).toContain("<li>項目</li>");
    expect(html).toContain("<code>code</code>");
    expect(STORY_MARKDOWN_ALLOWED_TAGS).toEqual(expect.arrayContaining(["em", "strong", "blockquote", "li", "code"]));
    expect(STORY_MARKDOWN_ALLOWED_ATTR).toEqual(["src", "alt"]);
  });

  // 卡片作者把配圖寫在開場白裡（實例：TestCards 的 furry-male-scenarios，30 個開場白共 32 張圖）
  it("keeps markdown images so card art shows up", () => {
    const html = renderStoryMarkdown("句子\n\n![image](https://static1.e621.net/data/84/55/x.jpg)");

    expect(html).toContain('<img src="https://static1.e621.net/data/84/55/x.jpg"');
    expect(html).toContain('alt="image"');
  });

  // DOMPurify 的 ALLOWED_URI_REGEXP 對 img src 攔不住這幾種，把關改做在 marked 的 renderer
  it("drops image sources that are not http(s) or data:image", () => {
    const script = renderStoryMarkdown("![替代文字](javascript:alert(1))");
    const relative = renderStoryMarkdown("![x](../../etc/passwd)");
    const dataText = renderStoryMarkdown("![x](data:text/html;base64,PHNjcmlwdD4=)");
    const dataImage = renderStoryMarkdown("![x](data:image/png;base64,iVBORw0KGgo=)");

    expect(script).not.toContain("<img");
    expect(script).toContain("替代文字");
    expect(relative).not.toContain("<img");
    expect(dataText).not.toContain("<img");
    expect(dataImage).toContain("data:image/png;base64,iVBORw0KGgo=");
  });

  it("shows raw HTML as text and removes non-allowlisted markup", () => {
    const scriptHtml = renderStoryMarkdown("<script>alert(1)</script>");
    const imageHtml = renderStoryMarkdown("<img src=x onerror=alert(1)>");
    const linkHtml = renderStoryMarkdown("[點我](javascript:alert(1))");
    const divHtml = renderStoryMarkdown("<div onclick=x>內容</div>");

    expect(scriptHtml).not.toContain("<script>");
    expect(scriptHtml).toMatch(/&lt;script&gt;alert\(1\)&lt;\/script&gt;/);
    expect(imageHtml).not.toContain("<img");
    expect(linkHtml).not.toContain("<a");
    expect(linkHtml).not.toContain("javascript:");
    expect(divHtml).not.toMatch(/<[^>]*\bonclick=/i);
  });
});

// 現在都是簡單 fixture，精確字串比對夠用；之後加畸形 HTML 或 SVG 案改驗 DOM 結構
describe("sanitizeStoryHtml", () => {
  it("strips handlers, non-allowlisted tags and script URIs on its own", () => {
    const html = sanitizeStoryHtml(
      '<p onclick="x" style="color:red" class="c">字</p><img src="javascript:alert(1)" onerror="x" alt="a">' +
        '<a href="https://e.com">連結</a><svg><script>alert(1)</script></svg><iframe src="https://e.com"></iframe>',
    );

    expect(html).toBe('<p>字</p><img alt="a">連結');
  });

  it("keeps https images", () => {
    expect(sanitizeStoryHtml('<img src="https://e.com/a.png" alt="a">')).toBe(
      '<img src="https://e.com/a.png" alt="a">',
    );
  });

  it("strips obfuscated javascript URIs", () => {
    const sources = [
      "JaVaScRiPt:alert(1)",
      "&#106;avascript:alert(1)",
      "&#x6A;&#x61;vascript:alert(1)",
      "java&#x09;script:alert(1)",
      "\u0001javascript:alert(1)",
      " \njavascript:alert(1)",
    ];

    for (const src of sources) {
      expect(sanitizeStoryHtml(`<img src="${src}" alt="a">`), src).toBe('<img alt="a">');
    }
  });

  it("is the exit renderStoryMarkdown actually goes through", () => {
    const sanitize = vi.spyOn(DOMPurify, "sanitize").mockReturnValue("SENTINEL" as never);

    expect(renderStoryMarkdown("*動作*")).toBe("SENTINEL");
    expect(sanitize).toHaveBeenCalledTimes(1);
    expect(sanitize).toHaveBeenCalledWith(expect.stringContaining("<em>動作</em>"), {
      ALLOWED_TAGS: STORY_MARKDOWN_ALLOWED_TAGS,
      ALLOWED_ATTR: STORY_MARKDOWN_ALLOWED_ATTR,
    });
  });
});
