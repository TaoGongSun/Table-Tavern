// @vitest-environment happy-dom

import { describe, expect, it } from "vitest";
import {
  renderStoryMarkdown,
  STORY_MARKDOWN_ALLOWED_ATTR,
  STORY_MARKDOWN_ALLOWED_TAGS,
} from "./story-markdown";

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
