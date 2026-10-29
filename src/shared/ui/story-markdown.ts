import DOMPurify, { type DOMPurify as Purifier } from "dompurify";
import { Marked, type Tokens } from "marked";
import { prepareRawHtml } from "./story-html-scan";
import { sanitizeInlineStyle } from "./story-inline-style";

// 訊息裡的原始 HTML 照 ST 放行再消毒（ST 是 DOMPurify 預設全清單，這裡只收排版類）：
// 名單內照渲染，名單外剝標籤留內容（DOMPurify 的 KEEP_CONTENT），漏列只少樣式、不丟字。
// 不收 a（連結只留文字，同 Markdown 連結）、表單、svg/math、影音、class/id；style 只留字色、字型類（見 story-inline-style）
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
  "b",
  "i",
  "u",
  "s",
  "strike",
  "del",
  "ins",
  "mark",
  "small",
  "sub",
  "sup",
  "span",
  "div",
  "center",
  "font",
  "details",
  "summary",
  "table",
  "thead",
  "tbody",
  "tfoot",
  "tr",
  "th",
  "td",
  "caption",
  "colgroup",
  "col",
  "dl",
  "dt",
  "dd",
  "q",
  "cite",
  "abbr",
  "kbd",
  "ruby",
  "rt",
  "rp",
  "figure",
  "figcaption",
];

// 危險的 on* 事件處理器不在名單內，DOMPurify 一律剝掉；src 只有 img 用得到，來源另由 hook 把關
export const STORY_MARKDOWN_ALLOWED_ATTR: string[] = [
  "src",
  "alt",
  "title",
  "color",
  "open",
  "colspan",
  "rowspan",
  "align",
  "style",
];

// 圖片來源只收 http(s) 與內嵌 data:image。DOMPurify 的 ALLOWED_URI_REGEXP 對 src 攔不住
// javascript: 與相對路徑（實測過，story-markdown.test.ts 有對應案例），所以 Markdown 圖片在
// renderer 擋、原始 HTML 的 img 在消毒後的 DOM 上逐張再查（見 settleImages）
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
    // 原始 HTML 放行給消毒器；已閉合的註解拿掉（同 ST：卡片的隱藏記憶欄玩家看不到，原文與上下文照留）。
    // 沒閉合的註解連同之後的內容跳脫成字：放行的話解析器會把它之後整則回覆吞掉
    html(token: Tokens.HTML | Tokens.Tag): string {
      const text = token.text.replace(/<!--(?:-?>|[\s\S]*?--!?>)/g, "");
      const unclosed = text.indexOf("<!--");
      return unclosed < 0 ? text : text.slice(0, unclosed) + escapeHtml(text.slice(unclosed));
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

// 剝掉時連內容一起丟的元素：只限內容本來就不該給玩家看的（程式、樣式、文件標頭類）。
// DOMPurify 預設還會丟 svg、math、影音、noscript 的內容，那些在故事裡是正文被包住（含沒收尾、
// 或被 </p> 提早關掉的 <svg a=x/>後文），改成剝標籤留字
export const STORY_DROPPED_CONTENTS = ["script", "style", "title", "template", "iframe", "noembed", "noframes", "xmp", "plaintext", "head"];

let storyPurifier: Purifier | null = null;

// 消毒器在自己的文件裡解析，沒有 window 可拿 HTMLUnknownElement 比對，改看內建型別標記
function isUnknownElement(node: Element): boolean {
  return Object.prototype.toString.call(node) === "[object HTMLUnknownElement]" || node.localName.includes("-");
}

const SHOW_TEXT = 4; // NodeFilter.SHOW_TEXT；消毒器的文件不一定有 window 可取常數

/** 文件順序的文字節點，pre／code 裡的不算 */
function proseTexts(root: Node): Text[] {
  const walker = (root.ownerDocument ?? (root as Document)).createTreeWalker(root, SHOW_TEXT);
  const texts: Text[] = [];
  while (walker.nextNode()) {
    const text = walker.currentNode as Text;
    if (!text.parentElement?.closest("pre, code")) texts.push(text);
  }
  return texts;
}

// 不認得的元素（<status>、<maintext>）剝掉後內容留在原處，裡面的換行照 ST 換成 <br>：
// 標籤獨佔一行起頭的 HTML 區塊 marked 不套 breaks，不換的話多行狀態欄會擠成一行。
// 只拆文字節點，pre／code 裡的不動；外層先處理過，裡層就沒有換行可拆
function breakLinesInUnknown(node: Element): void {
  const document = node.ownerDocument;
  const texts = proseTexts(node);
  // 同 ST 先修剪頭尾：<status>\n…\n</status> 不要多出空行；巢狀時修到第一段真正的字為止
  for (const text of texts) if ((text.data = text.data.trimStart())) break;
  for (const text of [...texts].reverse()) if ((text.data = text.data.trimEnd())) break;
  for (const text of texts) {
    if (!text.data.includes("\n")) continue;
    const pieces = text.data.split("\n");
    const fragment = document.createDocumentFragment();
    pieces.forEach((piece, index) => {
      if (piece) fragment.append(piece);
      if (index < pieces.length - 1) fragment.append(document.createElement("br"));
    });
    text.replaceWith(fragment);
  }
}

// 卡片自訂的中文標籤（<核心记忆>…</核心记忆>）：HTML 的標籤名得是英文字母開頭，這種在解析器眼中
// 是文字，ST 實際是開頭露字、結尾被吃掉。這裡比 ST 好一步：有同名結尾的成對標記兩端都拿掉、內容原樣；
// 沒結尾的（含串流中還沒到的）照顯示。只認名稱第一字是非 ASCII 字母，<3>、a<b 不受影響。
// 在消毒後的 DOM 上做：只看 pre／code 以外的文字節點，屬性與程式碼天然碰不到，跨行內程式碼也配得到
const CJK_MARK = /<(\/?)((?![\x00-\x7F])\p{L}[^\s<>/&"'=]{0,40})\s*>/gu;

type Mark = { text: Text; start: number; end: number; name: string; closing: boolean };

function stripPairedCjkTags(root: Node): void {
  const marks: Mark[] = [];
  for (const text of proseTexts(root)) {
    for (const match of text.data.matchAll(CJK_MARK)) {
      marks.push({ text, start: match.index, end: match.index + match[0].length, name: match[2], closing: !!match[1] });
    }
  }
  const open: Mark[] = [];
  const drop: Mark[] = [];
  for (const mark of marks) {
    if (!mark.closing) {
      open.push(mark);
      continue;
    }
    for (let depth = open.length - 1; depth >= 0; depth--) {
      if (open[depth].name !== mark.name) continue;
      drop.push(open[depth], mark);
      open.length = depth;
      break;
    }
  }
  const byText = new Map<Text, Mark[]>();
  for (const mark of drop) byText.set(mark.text, [...(byText.get(mark.text) ?? []), mark]);
  for (const [text, list] of byText) {
    const original = text.data;
    // 標記獨佔一行時，marked 在它旁邊留的 <br> 一起拿掉，免得內容前後多一行空白
    for (const mark of list) {
      if (!mark.closing && original.slice(mark.end).trim() === "") dropBr(text.nextSibling);
      if (mark.closing && original.slice(0, mark.start).trim() === "") dropBr(text.previousSibling);
    }
    // 由後往前刪，前面標記的位置才不會跑掉
    let data = original;
    for (const mark of list.sort((x, y) => y.start - x.start)) data = data.slice(0, mark.start) + data.slice(mark.end);
    text.data = data;
  }
}

function dropBr(node: ChildNode | null): void {
  if (node?.nodeName === "BR") node.remove();
}

export type StoryRenderOptions = {
  /** 額外的圖片來源條件（網頁版：只收同源與 data:），通過基本的 http(s)／data:image 檢查後才問 */
  imageAllowed?: (src: string) => boolean;
  /** 擋下的圖片換成什麼；預設是替代文字 */
  blockedImage?: (alt: string, document: Document) => Node;
};

// 原始 HTML 的圖片照 Markdown 圖片同一道來源門：缺 src、空 src、不合規的整張換掉，不留網址。
// 在消毒器自己的惰性文件上做，換掉之前不會有任何一張圖被載入
function settleImages(root: Element, options: StoryRenderOptions): void {
  for (const image of [...root.querySelectorAll("img")]) {
    const src = image.getAttribute("src") ?? "";
    if (ALLOWED_IMAGE_SRC.test(src) && (options.imageAllowed?.(src) ?? true)) continue;
    const alt = image.getAttribute("alt") ?? "";
    const document = image.ownerDocument;
    image.replaceWith(options.blockedImage?.(alt, document) ?? document.createTextNode(alt));
  }
}

// 專用實例：hook 只掛在故事渲染，不影響其他地方的 DOMPurify
export function getStoryPurifier(): Purifier {
  if (storyPurifier) return storyPurifier;
  const purifier = DOMPurify(window);
  purifier.addHook("uponSanitizeElement", (node, data) => {
    if (node.nodeType === 1 && !data.allowedTags[data.tagName] && isUnknownElement(node as Element)) {
      breakLinesInUnknown(node as Element);
    }
  });
  purifier.addHook("uponSanitizeAttribute", (_node, data) => {
    if (data.attrName !== "style") return;
    data.attrValue = sanitizeInlineStyle(data.attrValue);
    if (!data.attrValue) data.keepAttr = false;
  });
  storyPurifier = purifier;
  return purifier;
}

// 故事 HTML 的最後一道出口：renderer 漏放什麼，這裡都要剝掉。
// 消毒後在消毒器的惰性文件上收尾（圖片來源、中文成對標籤），只刪字與換掉節點，不會生出新標記
export function sanitizeStoryHtml(html: string, options: StoryRenderOptions = {}): string {
  const body = getStoryPurifier().sanitize(html, {
    ALLOWED_TAGS: STORY_MARKDOWN_ALLOWED_TAGS,
    ALLOWED_ATTR: STORY_MARKDOWN_ALLOWED_ATTR,
    ALLOW_DATA_ATTR: false,
    ALLOW_ARIA_ATTR: false,
    FORBID_CONTENTS: STORY_DROPPED_CONTENTS,
    RETURN_DOM: true,
  }) as HTMLElement;
  settleImages(body, options);
  stripPairedCjkTags(body);
  return body.innerHTML;
}

export function renderStoryMarkdown(text: string, options: StoryRenderOptions = {}): string {
  return sanitizeStoryHtml(prepareRawHtml(marked.parse(text) as string), options);
}
