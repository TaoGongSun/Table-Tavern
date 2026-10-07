// 宿主頁的故事渲染：桌面版安全渲染（DOMPurify，只留 img 的 src／alt）之外再擋外部圖片——
// 卡片的顯示 regex 能把 {{lastUserMessage}} 之類的巨集塞進圖片網址，一載圖就把對話送出去。
// 宿主頁只載 data: 與同源圖片；外部圖片換成替代文字，不留網址。CSP 的 img-src 是第二層。
import { renderStoryMarkdown } from "@desktop/shared/ui/story-markdown";

const IMG = /<img\b[^>]*>/gi;
const attr = (tag: string, name: string) => new RegExp(`\\s${name}="([^"]*)"`, "i").exec(tag)?.[1] ?? "";

function decodeAttr(value: string): string {
  return value.replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&");
}

export function isHostImageAllowed(src: string, origin: string): boolean {
  if (/^data:image\//i.test(src)) return true;
  try {
    return new URL(src, origin).origin === origin;
  } catch {
    return false;
  }
}

export function blockExternalImages(html: string, origin: string): string {
  return html.replace(IMG, (tag) => {
    if (isHostImageAllowed(decodeAttr(attr(tag, "src")), origin)) return tag;
    // alt 已是屬性跳脫過的文字，直接放進文字節點仍是安全的
    const alt = attr(tag, "alt");
    return `<span class="image-blocked">[${alt || "image"}]</span>`;
  });
}

const hostOrigin = () => (typeof location !== "undefined" ? location.origin : "null");

export function renderHostMarkdown(text: string): string {
  return blockExternalImages(renderStoryMarkdown(text), hostOrigin());
}
