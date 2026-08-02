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
    expect(STORY_MARKDOWN_ALLOWED_ATTR).toEqual([]);
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
