// 宿主頁的故事渲染：桌面版安全渲染（DOMPurify 白名單）之外再擋外部圖片——
// 卡片的顯示 regex 能把 {{lastUserMessage}} 之類的巨集塞進圖片網址，一載圖就把對話送出去。
// 宿主頁只載 data: 與同源圖片；外部圖片換成替代文字，不留網址。判斷在消毒後的 DOM 上逐張做
// （不對 HTML 字串跑 regex：屬性值裡的 > 會讓它截錯），CSP 的 img-src 是第二層。
import { renderStoryMarkdown } from "@desktop/shared/ui/story-markdown";

export function isHostImageAllowed(src: string, origin: string): boolean {
  if (/^data:image\//i.test(src)) return true;
  if (!src.trim()) return false;
  try {
    return new URL(src, origin).origin === origin;
  } catch {
    return false;
  }
}

export function renderHostMarkdownAt(text: string, origin: string): string {
  return renderStoryMarkdown(text, {
    imageAllowed: (src) => isHostImageAllowed(src, origin),
    blockedImage: (alt, document) => {
      const note = document.createElement("span");
      note.className = "image-blocked";
      note.textContent = `[${alt || "image"}]`;
      return note;
    },
  });
}

const hostOrigin = () => (typeof location !== "undefined" ? location.origin : "null");

export function renderHostMarkdown(text: string): string {
  return renderHostMarkdownAt(text, hostOrigin());
}
