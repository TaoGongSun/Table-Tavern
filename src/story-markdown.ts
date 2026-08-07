import DOMPurify from "dompurify";
import { Marked, type Tokens } from "marked";

export const STORY_MARKDOWN_ALLOWED_TAGS = [
  "em",
  "strong",
  "code",
  "pre",
  "blockquote",
  "ul",
  "ol",
  "li",
  "p",
  "br",
  "hr",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  // 卡片作者常把配圖寫在開場白裡（Markdown 圖片語法，圖存在他們自己的圖床），
  // 不放行的話那幾行整段消失、開場白讀起來是斷的
  "img",
];

// src／alt 只有 img 用得到；白名單其餘標籤都不吃這兩個屬性。
// 危險的 on* 事件處理器不在名單內，DOMPurify 一律剝掉。
export const STORY_MARKDOWN_ALLOWED_ATTR: string[] = ["src", "alt"];

// 圖片來源只收 http(s) 與內嵌 data:image。把關做在 renderer（見下面的 image）而不是
// 交給 DOMPurify 的 ALLOWED_URI_REGEXP——那個選項對 src 攔不住 javascript: 與相對路徑，
// 實測過（story-markdown.test.ts 有對應案例）
const ALLOWED_IMAGE_SRC = /^(?:https?:\/\/|data:image\/)/i;

function escapeHtml(text: string): string {
  return text.replace(/[&<>"]/g, (character) => {
    switch (character) {
      case "&":
        return "&amp;";
      case "<":
        return "&lt;";
      case ">":
        return "&gt;";
      case '"':
        return "&quot;";
      default:
        return character;
    }
  });
}

const marked = new Marked({
  breaks: true,
  gfm: true,
  renderer: {
    html(token: Tokens.HTML | Tokens.Tag): string {
      return escapeHtml(token.text);
    },
    link(token): string {
      return token.text;
    },
    // 卡片是不可信來源：來源不合規（javascript:、指向本機檔案的相對路徑）就連標籤都不產出，
    // 退回原本的替代文字，玩家至少看得到那裡本來有張圖
    image(token: Tokens.Image): string {
      if (!ALLOWED_IMAGE_SRC.test(token.href)) return escapeHtml(token.text);
      return `<img src="${escapeHtml(token.href)}" alt="${escapeHtml(token.text)}">`;
    },
  },
});

export function renderStoryMarkdown(text: string): string {
  const html = marked.parse(text) as string;
  return DOMPurify.sanitize(html, {
    ALLOWED_TAGS: STORY_MARKDOWN_ALLOWED_TAGS,
    ALLOWED_ATTR: STORY_MARKDOWN_ALLOWED_ATTR,
  });
}
