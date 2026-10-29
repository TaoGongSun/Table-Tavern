// 真實 WebKit（Tauri macOS 的引擎）驗 DOMPurify 這道出口本身：npm run test:webkit。
// happy-dom 跑 DOMPurify 會拆掉第一個元素、放過後面的事件屬性（3.4.12–3.4.16 皆然），
// 所以 story-markdown.test.ts 改用 jsdom 驗同一個函式；這支留著對照真引擎。
import { expect, it } from "vitest";
import { renderStoryMarkdown, sanitizeStoryHtml } from "./story-markdown";

it("sanitizer config strips handlers, non-allowlisted tags and script URIs on its own", () => {
  const html = sanitizeStoryHtml(
    '<p onclick="x" style="color:red" class="c">字</p><img src="javascript:alert(1)" onerror="x" alt="a">' +
      '<a href="https://e.com">連結</a><svg><script>alert(1)</script></svg><iframe src="https://e.com"></iframe>',
  );

  expect(html).toBe('<p style="color: red">字</p>a連結');
});

it("story renderer hooks behave the same in the real engine", () => {
  expect(renderStoryMarkdown("<status>\nHP:10\nMP:5\n</status>")).toBe("HP:10<br>MP:5");
  expect(renderStoryMarkdown('<img src="../x.png" alt="替代"><span data-x="1" aria-label="y">s</span>')).toBe(
    "<p>替代<span>s</span></p>\n",
  );
  expect(renderStoryMarkdown("<核心记忆>记得</核心记忆>")).toBe("<p>记得</p>\n");
  expect(renderStoryMarkdown("<甲>前`code`後</甲>")).toBe("<p>前<code>code</code>後</p>\n");
  const box = document.createElement("div");
  box.innerHTML = renderStoryMarkdown('<div title="<甲>提示</甲>">正文</div><pre><甲>字</甲></pre >');
  expect(box.querySelector("div")?.getAttribute("title")).toBe("<甲>提示</甲>");
  expect(box.querySelector("pre")?.textContent).toBe("<甲>字</甲>");
  box.innerHTML = renderStoryMarkdown('<script>const x="<script>";</script>後文');
  expect(box.textContent?.trim()).toBe("後文");
  box.innerHTML = renderStoryMarkdown("<script>內文</script 後文");
  expect(box.textContent).toContain("後文");
  box.innerHTML = renderStoryMarkdown("<svg><svg/></svg>後文");
  expect(box.textContent?.trim()).toBe("後文");
  box.innerHTML = renderStoryMarkdown('<script>內文</script x=">後文');
  expect(box.textContent).toContain("後文");
  box.innerHTML = renderStoryMarkdown("<svg><svg a=x/></svg>後文");
  expect(box.textContent).toContain("後文");
});

const STRUCTURE_PROBES = [
  '<svg><foreignObject><b>粗</b><style>p{color:red}</style></foreignObject></svg>後文',
  '<svg><style>a{}</style><animate attributeName="href" to="javascript:alert(1)"/></svg>後文',
  '<math><mglyph><style><img src=x onerror=alert(1)></style></mglyph></math>後文',
  '<math><annotation-xml encoding="text/html"><style><img src=x onerror=alert(1)></style></annotation-xml></math>後文',
  '<noscript><p title="</noscript><img src=x onerror=alert(1)>"></noscript>後文',
  '<svg></p><style><a id="</style><img src=x onerror=alert(1)>"></a></style></svg>後文',
  '<form><math><mtext></form><form><mglyph><style></math><img src=x onerror=alert(1)>',
];
const DANGEROUS = "script, style, svg, math, animate, foreignObject, mglyph, annotation-xml, noscript, form, [onerror], [style]";

it("structure probes leave no dangerous elements in the real engine", () => {
  for (const raw of STRUCTURE_PROBES) {
    const box = document.createElement("div");
    box.innerHTML = renderStoryMarkdown(raw);
    expect(box.querySelectorAll(DANGEROUS), raw).toHaveLength(0);
    expect(box.querySelectorAll("img"), raw).toHaveLength(0);
    if (raw.endsWith("後文")) expect(box.textContent, raw).toContain("後文");
  }
  const box = document.createElement("div");
  box.innerHTML = renderStoryMarkdown("&#xE000;tt-story-end&#xE001;<script>後文");
  expect(box.textContent).toContain("後文");
});

it("inline style colors apply and nested font sizes stay within 8–32px", () => {
  const box = document.createElement("div");
  box.style.fontSize = "16px";
  document.body.append(box);
  const nested = (style: string) => `${`<span style="${style}">`.repeat(5)}字${"</span>".repeat(5)}`;
  const innermost = (style: string) => {
    box.innerHTML = renderStoryMarkdown(nested(style));
    const spans = box.querySelectorAll("span");
    return getComputedStyle(spans[spans.length - 1]).fontSize;
  };
  for (const style of ["font-size:2em", "font-size:200%", "font-size:larger", "font-size:100px"]) {
    expect(innermost(style), style).toBe("32px");
  }
  for (const style of ["font-size:0.5em", "font-size:50%", "font-size:smaller", "font-size:1px"]) {
    expect(innermost(style), style).toBe("8px");
  }
  box.innerHTML = renderStoryMarkdown('<span style="color:red;background-color:rgba(0,0,0,.5)">字</span>');
  const span = getComputedStyle(box.querySelector("span")!);
  expect([span.color, span.backgroundColor]).toEqual(["rgb(255, 0, 0)", "rgba(0, 0, 0, 0.5)"]);
  box.remove();
});
