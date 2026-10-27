// @vitest-environment jsdom
// 不能用 happy-dom：DOMPurify 在 happy-dom 下不消毒（拆掉第一個元素、放過事件屬性與 javascript:），
// 測不到 sanitizeStoryHtml 這層。真 WebKit 對照見 story-markdown.webkit.test.tsx。

import { afterEach, describe, expect, it, vi } from "vitest";
import {
  getStoryPurifier,
  renderStoryMarkdown,
  sanitizeStoryHtml,
  STORY_MARKDOWN_ALLOWED_ATTR,
  STORY_DROPPED_CONTENTS,
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
    expect(STORY_MARKDOWN_ALLOWED_ATTR).not.toEqual(expect.arrayContaining(["href", "style", "class", "id"]));
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

  it("drops scripts, handlers and non-allowlisted markup from raw HTML", () => {
    const scriptHtml = renderStoryMarkdown("<script>alert(1)</script>後文");
    const imageHtml = renderStoryMarkdown("<img src=x onerror=alert(1)>");
    const linkHtml = renderStoryMarkdown("[點我](javascript:alert(1))");
    const divHtml = renderStoryMarkdown("<div onclick=x>內容</div>");

    expect(scriptHtml).not.toMatch(/<script|alert/);
    expect(scriptHtml).toContain("後文");
    expect(imageHtml).not.toContain("<img");
    expect(linkHtml).not.toContain("<a");
    expect(linkHtml).not.toContain("javascript:");
    expect(divHtml).toBe("<div>內容</div>");
  });
});

// 對照 ST（showdown 放行原始 HTML＋DOMPurify）：名單內照渲染，名單外剝標籤留內容
describe("raw HTML", () => {
  const dom = (text: string) => {
    const box = document.createElement("div");
    box.innerHTML = renderStoryMarkdown(text);
    return box;
  };

  it("renders allowlisted formatting tags the way ST does", () => {
    const html = renderStoryMarkdown(
      '<b>粗</b><i>斜</i><u>底</u><s>刪</s><font color="red">紅</font><center>中</center>' +
        "<details><summary>點開</summary>藏的</details>",
    );

    expect(html).toContain("<b>粗</b><i>斜</i><u>底</u><s>刪</s>");
    expect(html).toContain('<font color="red">紅</font>');
    expect(html).toContain("<center>中</center>");
    expect(html).toContain("<details><summary>點開</summary>藏的</details>");
  });

  it("renders markdown tables and strikethrough that used to lose their tags", () => {
    const html = renderStoryMarkdown("~~刪~~\n\n| a | b |\n|---|---|\n| 1 | 2 |");

    expect(html).toContain("<del>刪</del>");
    expect(dom("| a | b |\n|---|---|\n| 1 | 2 |").querySelectorAll("table th, table td")).toHaveLength(4);
  });

  it("strips unknown tags and keeps their content", () => {
    expect(dom("<thinking>想法</thinking>正文").textContent?.trim()).toBe("想法正文");
    expect(renderStoryMarkdown("<maintext>正文</maintext>")).not.toContain("maintext");
  });

  it("strips attributes outside the allowlist, including data-* and aria-*", () => {
    const html = renderStoryMarkdown(
      '<span data-x="1" aria-label="y" class="btn" id="root" style="position:fixed" onmouseover="x()" title="提示">s</span>',
    );

    expect(html).toContain('<span title="提示">s</span>');
  });

  it("keeps links as plain text whether written in markdown or raw HTML", () => {
    const raw = renderStoryMarkdown('<a href="https://e.com" target="_blank">連結</a>');
    const markdown = renderStoryMarkdown("[連結](https://e.com)");

    expect(raw).not.toMatch(/<a\b|href/);
    expect(raw).toContain("連結");
    expect(markdown).toBe(raw);
  });

  it("holds raw HTML images to the same sources as markdown images", () => {
    const sources = ["javascript:alert(1)", "../../etc/passwd", "file:///etc/passwd", "//evil.example/a.png", "data:text/html;base64,PHNjcmlwdD4="];
    for (const src of sources) {
      const raw = renderStoryMarkdown(`<img src="${src}" alt="替代">`);
      const markdown = renderStoryMarkdown(`![替代](${src})`);
      expect(raw, src).not.toContain("<img");
      expect(raw, src).toContain("替代");
      expect(markdown, src).not.toContain("<img");
    }
    expect(renderStoryMarkdown('<img src="https://e.com/a.png" alt="a">')).toContain('<img src="https://e.com/a.png" alt="a">');
    expect(renderStoryMarkdown('<img src="data:image/png;base64,iVBORw0KGgo=" alt="a">')).toContain("data:image/png");
  });

  it("does not let a rejected image's alt become markup", () => {
    const html = renderStoryMarkdown('<img src="javascript:x" alt="<b onmouseover=x>粗</b>">');

    expect(html).not.toMatch(/<b/);
    expect(dom('<img src="javascript:x" alt="<b onmouseover=x>粗</b>">').querySelector("b")).toBeNull();
    expect(dom('<img src="javascript:x" alt="<b>粗</b>">').textContent?.trim()).toBe("<b>粗</b>");
  });

  // 同 ST：標籤獨佔一行起頭的 HTML 區塊 marked 不套 breaks，換行要在不認得的元素裡補成 <br>
  it("turns line breaks inside unknown elements into <br> but leaves pre and code alone", () => {
    expect(renderStoryMarkdown("<status>\nHP:10\nMP:5\n</status>\n\n下一段")).toBe("HP:10<br>MP:5<p>下一段</p>\n");
    expect(renderStoryMarkdown("<status>\n<pre>a\nb</pre>\nc\n</status>")).toContain("<pre>a\nb</pre>");
    expect(renderStoryMarkdown("<status>\n<code>a\nb</code>\n</status>")).toContain("<code>a\nb</code>");
    expect(renderStoryMarkdown("<outer>\n<inner>\na\nb\n</inner>\n</outer>")).toBe("a<br>b");
  });

  it("keeps newlines collapsed inside known block tags, like ST", () => {
    expect(renderStoryMarkdown("<div>\n一\n二\n</div>")).toBe("<div>\n一\n二\n</div>");
  });

  it("shows an unclosed swallowing tag as text instead of eating the rest of the reply", () => {
    for (const tag of ["script", "style", "textarea", "title", "plaintext", "iframe", "xmp", "template"]) {
      const shown = dom(`前<${tag}>中段\n\n後面的故事`).textContent ?? "";
      expect(shown, tag).toContain(`<${tag}>中段`);
      expect(shown, tag).toContain("後面的故事");
    }
    // svg／math 在段落裡會被 </p> 提早關掉：標記不顯示，但裡面的字照留
    for (const tag of ["svg", "math", "video", "audio", "noscript"]) {
      const shown = dom(`前<${tag}>中段\n\n後面的故事`).textContent ?? "";
      expect(shown, tag).toContain("中段");
      expect(shown, tag).toContain("後面的故事");
    }
    const block = dom("<svg>\n中段\n\n<b>後面的故事</b>");
    expect(block.textContent).toContain("中段");
    expect(block.querySelector("b")?.textContent).toBe("後面的故事");
  });

  it("strips closed svg markup but keeps its words", () => {
    expect(dom('<svg><text x="0">圖上字</text><script>alert(1)</script></svg>後文').textContent?.trim()).toBe("圖上字後文");
  });

  it("only escapes the unclosed tag, not closed ones next to it", () => {
    const shown = dom("<script>x</script>前<plaintext>中\n\n<style>p{}</style>後").textContent ?? "";
    expect(shown).not.toMatch(/<script|<style|p\{\}/);
    expect(shown).toContain("<plaintext>中");
    expect(shown).toContain("後");
  });

  it("treats raw-text element content as text when looking for the end tag", () => {
    expect(dom('<script>const x="<script>";</script>後文').textContent?.trim()).toBe("後文");
    expect(dom('<style>a::after{content:"<style>"}</style>後文').textContent?.trim()).toBe("後文");
  });

  it("only counts a complete end tag as closing a raw-text element", () => {
    expect(dom("<script>內文</script 後文").textContent).toContain("後文");
    const full = "<script>x</script >後文";
    for (let end = "<script>x".length; end <= full.length; end++) {
      const prefix = full.slice(0, end);
      const shown = dom(prefix).textContent ?? "";
      if (prefix.includes("</script >")) expect(shown, prefix).not.toMatch(/<script|x/);
      else expect(shown, prefix).toContain("<script>x");
      if (prefix.endsWith("後")) expect(shown, prefix).toContain("後");
    }
    expect(dom("<style>p{}</style/>後文").textContent?.trim()).toBe("後文");
    for (const tag of ["script", "style"]) {
      expect(dom(`<${tag}>內文</${tag} x=">後文`).textContent, tag).toContain("後文");
    }
  });

  it("treats self-closing svg and math as closed", () => {
    for (const raw of ["<svg/>後文", "<svg />後文", "<svg><svg/></svg>後文", "<math/>後文"]) {
      const shown = dom(raw).textContent ?? "";
      expect(shown.trim(), raw).toBe("後文");
    }
    expect(dom("<template/>中\n\n後文").textContent).toContain("後文");
    expect(dom("<svg a=x/>後文").textContent).toContain("後文");
    expect(dom("<svg><svg a=x/></svg>後文").textContent).toContain("後文");
  });

  it("does not mistake a tag-like attribute value for a tag", () => {
    const shown = dom('<span title="<svg>">字</span>後文').textContent ?? "";
    expect(shown.trim()).toBe("字後文");
  });

  it("drops closed style blocks without showing the CSS", () => {
    expect(dom("<style>p{color:red}</style>正文").textContent?.trim()).toBe("正文");
  });

  it("leaves bare less-than signs alone", () => {
    expect(dom("a<b 且 <3 與 <３>").textContent?.trim()).toBe("a<b 且 <3 與 <３>");
  });
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

// 外來元素與 noscript 的經典結構攻擊：輸出只能有白名單元素、沒有事件屬性，正文照留
describe("structure probes", () => {
  it("leaves no dangerous elements or handlers behind", () => {
    for (const raw of STRUCTURE_PROBES) {
      const box = document.createElement("div");
      box.innerHTML = renderStoryMarkdown(raw);
      expect(box.querySelectorAll(DANGEROUS), raw).toHaveLength(0);
      expect(box.querySelectorAll("img"), raw).toHaveLength(0);
      if (raw.endsWith("後文")) expect(box.textContent, raw).toContain("後文");
    }
  });

  // 輸入冒充不了修復用的哨兵：實體編碼後解析出來的字和固定哨兵一樣也沒用
  it("cannot be fooled by an entity-encoded sentinel lookalike", () => {
    const lookalikes = [
      "&#xE000;tt-story-end&#xE001;",
      "&#57344;tt-story-end&#57345;",
      "\uE000tt-story-end\uE001",
      "tt-end-&#x30;&#48;&amp;tt-end-",
    ];
    for (const fake of lookalikes) {
      const box = document.createElement("div");
      box.innerHTML = renderStoryMarkdown(`${fake}<script>後文`);
      expect(box.textContent, fake).toContain("後文");
      expect(box.textContent, fake).not.toMatch(/tt-end-[0-9a-f]{32}/);
    }
  });
});

// 卡片自訂的中文標籤：解析器不當它是標籤，ST 實際是開頭露字、結尾被吃；這裡成對的兩端都拿掉
describe("custom tags with non-ASCII names", () => {
  const visible = (text: string) => {
    const box = document.createElement("div");
    box.innerHTML = renderStoryMarkdown(text);
    return (box.textContent ?? "").trim();
  };

  it("removes both ends of a paired tag and keeps the content", () => {
    expect(visible("<核心记忆>角色记得A事</核心记忆>尾")).toBe("角色记得A事尾");
    expect(renderStoryMarkdown("<开局>\n第一行\n第二行\n</开局>")).toBe("<p>第一行<br>第二行</p>\n");
  });

  it("pairs nested and multi-paragraph tags by name", () => {
    expect(visible("<外><內>字</內>中</外>")).toBe("字中");
    const shown = visible("<状态>\n\n- 项目\n\n</状态>\n\n後文");
    expect(shown).not.toMatch(/状态/);
    expect(shown).toContain("项目");
    expect(shown).toContain("後文");
  });

  it("keeps an opening tag with no matching end, including while it streams in", () => {
    expect(visible("<甲><乙>內</甲>")).toBe("<乙>內");
    const full = "<核心记忆>记得</核心记忆>";
    for (let end = 1; end <= full.length; end++) {
      const prefix = full.slice(0, end);
      const shown = visible(prefix);
      if (prefix.endsWith("</核心记忆>")) expect(shown, prefix).toBe("记得");
      else expect(shown, prefix).toContain("记得".slice(0, Math.max(0, end - 6)));
    }
  });

  it("pairs across inline code without touching the code", () => {
    expect(visible("<甲>前`code`後</甲>")).toBe("前code後");
    expect(renderStoryMarkdown("<甲>前`<乙>x</乙>`後</甲>")).toContain("<code>&lt;乙&gt;x&lt;/乙&gt;</code>");
  });

  it("never edits attribute values", () => {
    const box = document.createElement("div");
    box.innerHTML = renderStoryMarkdown('<div title="<甲>提示</甲>">正文</div>');
    expect(box.querySelector("div")?.getAttribute("title")).toBe("<甲>提示</甲>");
    expect(box.textContent).toBe("正文");
  });

  it("leaves raw HTML pre content alone, even with a spaced end tag", () => {
    expect(visible("<pre><甲>字</甲></pre >")).toBe("<甲>字</甲>");
  });

  it("leaves tags inside code untouched", () => {
    expect(visible("`<核心>x</核心>`")).toBe("<核心>x</核心>");
    expect(visible("```\n<核心>x</核心>\n```")).toContain("<核心>x</核心>");
    expect(visible("段落\n\n    <核心>x</核心>\n")).toContain("<核心>x</核心>");
  });

  it("works inside raw HTML blocks too", () => {
    expect(visible("<div>\n<开局>內容</开局>\n</div>").trim()).toBe("內容");
  });

  it("ignores names that start with a digit or punctuation", () => {
    expect(visible("<3>愛</3>")).toBe("<3>愛</3>");
    expect(visible("<「引」>字</「引」>")).toContain("「引」");
  });
});

// 現在都是簡單 fixture，精確字串比對夠用；之後加畸形 HTML 或 SVG 案改驗 DOM 結構
describe("sanitizeStoryHtml", () => {
  it("strips handlers, non-allowlisted tags and script URIs on its own", () => {
    const html = sanitizeStoryHtml(
      '<p onclick="x" style="color:red" class="c">字</p><img src="javascript:alert(1)" onerror="x" alt="a">' +
        '<a href="https://e.com">連結</a><svg><script>alert(1)</script></svg><iframe src="https://e.com"></iframe>',
    );

    expect(html).toBe("<p>字</p>a連結");
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
      expect(sanitizeStoryHtml(`<img src="${src}" alt="a">`), src).toBe("a");
    }
  });

  it("is the exit renderStoryMarkdown actually goes through", () => {
    const body = document.createElement("body");
    body.textContent = "SENTINEL";
    const sanitize = vi.spyOn(getStoryPurifier(), "sanitize").mockReturnValue(body as never);

    expect(renderStoryMarkdown("*動作*")).toBe("SENTINEL");
    expect(sanitize).toHaveBeenCalledTimes(1);
    expect(sanitize).toHaveBeenCalledWith(expect.stringContaining("<em>動作</em>"), {
      ALLOWED_TAGS: STORY_MARKDOWN_ALLOWED_TAGS,
      ALLOWED_ATTR: STORY_MARKDOWN_ALLOWED_ATTR,
      ALLOW_DATA_ATTR: false,
      ALLOW_ARIA_ATTR: false,
      FORBID_CONTENTS: STORY_DROPPED_CONTENTS,
      RETURN_DOM: true,
    });
  });
});

// 對照 ST（showdown＋DOMPurify）：已閉合的註解畫面上看不到，code 裡的、沒閉合的照顯示
describe("HTML comments", () => {
  const visible = (text: string) => {
    const box = document.createElement("div");
    box.innerHTML = renderStoryMarkdown(text);
    return box.textContent ?? "";
  };

  it("hides the card's hidden memory bar written as stand-alone comment lines", () => {
    const shown = visible(
      "正文。\n\n<!-- 隐藏记忆栏 开始 -->\n\n<!-- \n<核心记忆>\n\n- 事件：[酒館遇襲]\n\n</核心记忆>\n\n-->\n\n<!-- 隐藏记忆栏 结束 -->\n\n選項：A／B",
    );
    expect(shown).toContain("正文。");
    expect(shown).toContain("選項：A／B");
    expect(shown).not.toMatch(/<!--|-->|记忆|酒館遇襲/);
  });

  it("hides inline and empty comments", () => {
    expect(visible("一行裡<!-- 行內 -->接著").trim()).toBe("一行裡接著");
    for (const empty of ["<!-->", "<!--->", "<!---->"]) {
      expect(visible(`前${empty}後`).trim(), empty).toBe("前後");
    }
  });

  it("ends a comment at the first terminator, including --!>", () => {
    expect(visible("前<!-- 外 <!-- 內 --> 剩下 -->後").trim()).toBe("前 剩下 -->後");
    expect(visible("<!-- a --!>後文<!-- b -->尾\n").trim()).toBe("後文尾");
  });

  it("keeps text after the comment on the same line", () => {
    expect(visible("<!-- x -->後文").trim()).toBe("後文");
  });

  it("keeps comments inside code", () => {
    expect(visible("`<!-- span -->`")).toContain("<!-- span -->");
    expect(visible("```\n<!-- fence -->\n```")).toContain("<!-- fence -->");
    expect(visible("段落\n\n    <!-- indented -->\n")).toContain("<!-- indented -->");
  });

  it("leaves an unclosed comment as text so the story is not swallowed", () => {
    const shown = visible("正文\n\n<!-- 沒關\n後面的故事");
    expect(shown).toContain("<!-- 沒關");
    expect(shown).toContain("後面的故事");
  });

  it("shows a comment while it streams in and hides it once closed", () => {
    const full = "正文\n\n<!-- 记忆 -->\n\n結尾";
    for (let end = 1; end <= full.length; end++) {
      const prefix = full.slice(0, end);
      const shown = visible(prefix);
      if (prefix.includes("-->")) expect(shown, prefix).not.toContain("记忆");
      else if (prefix.includes("<!-- 记")) expect(shown, prefix).toContain("记");
    }
  });
});
